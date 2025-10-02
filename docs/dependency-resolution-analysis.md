# Dependency Resolution System Analysis

## Problem Statement

Dependent tickets (like TVR-TEST-001) remain in `dependency_status='blocked'` even after all their dependencies (TVR-BE-001, TVR-FE-001) are closed.

## Expected Behavior

When a ticket is closed:
1. Its `dependency_status` should be set to `'ready'` (allowing dependents to proceed)
2. All tickets that depend on it should be checked
3. If all dependencies are satisfied, dependent tickets should be unblocked and resubmitted

## Actual Code Flow

### 1. Ticket Closure (`src/database/tickets.rs:364`)

```rust
pub async fn close_ticket(pool: &DbPool, ticket_id: &str, status: &str) -> Result<Option<Ticket>> {
    // Sets dependency_status = 'ready' for completed tickets
    let dep_status = if status == "Completed" {
        "ready"
    } else {
        "blocked"
    };

    // Updates ticket with new dependency_status
    sqlx::query("UPDATE tickets SET ... dependency_status = ?4 ...")
}
```

✅ **This is correct** - closed tickets get `dependency_status='ready'`

### 2. Dependency Cascade Trigger (`src/workers/queue.rs:742`)

```rust
async fn complete_ticket_with_cascade(...) {
    // Close ticket (sets dependency_status='ready')
    Ticket::close_ticket(&self.db, ticket_id, resolution).await?;

    // Trigger dependency cascade
    self.check_and_unblock_dependents(ticket_id).await?;  // Line 742
}
```

✅ **This is correct** - cascade is triggered after closure

### 3. Dependency Check Logic (`src/workers/dependencies.rs:25-145`)

```rust
pub async fn check_and_unblock_dependents(...) -> Result<()> {
    // Find all tickets that depend on the completed ticket
    let dependent_tickets = sqlx::query_as::<_, Ticket>(
        "SELECT ... WHERE td.parent_ticket_id = ?1
         AND t.state = 'open'
         AND t.dependency_status = 'blocked'"
    )
    .fetch_all(db).await?;

    for dependent_ticket in dependent_tickets {
        // Check if all dependencies are satisfied
        let blocking_dependencies = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM ticket_dependencies td
             INNER JOIN tickets dep ON td.parent_ticket_id = dep.ticket_id
             WHERE td.child_ticket_id = ?1
             AND dep.state != 'closed'"
        )
        .fetch_one(db).await?;

        if blocking_dependencies == 0 {
            // Unblock ticket
            sqlx::query(
                "UPDATE tickets SET dependency_status = 'ready',
                 updated_at = datetime('now') WHERE ticket_id = ?1"
            )
            .execute(db).await?;

            // Resubmit to queue
            Self::resubmit_parent_ticket(...).await?;
        }
    }
}
```

## Root Cause Analysis

The logic appears correct, but there are potential race conditions and edge cases:

### Issue 1: Race Condition During Planning Completion

When planning creates multiple tickets with dependencies:

1. **Planning ticket closes** → calls `check_and_unblock_dependents`
2. **Child tickets are created** with `dependency_status='blocked'`
3. **Dependency check happens** before children are even created
4. **Result**: Children remain blocked even though planning is complete

**Evidence**: Line 828-832 in `queue.rs`
```rust
// Step 3: Close planning ticket
Ticket::close_ticket(&self.db, planning_ticket_id.as_str(), "planning_complete").await?;

// Step 4: Auto-enqueue ready child tickets
self.enqueue_ready_child_tickets(&created_ticket_ids).await?;
```

The planning ticket is closed **before** checking which children are ready!

### Issue 2: Dependency Status Not Set During Creation

When tickets are created in `create_child_tickets_transactional`, what's the initial `dependency_status`?

Let me check the ticket creation code...

### Issue 3: Missing Dependency Resolution After Ticket Creation

The `execute_planning_completion` function:
1. Creates child tickets with dependencies
2. Closes planning ticket
3. Enqueues "ready" tickets

But it **never checks** if closing the planning ticket unblocks any of the newly created children!

## Proposed Solution

### Fix 1: Check Dependencies After Planning Completion

In `execute_planning_completion` (queue.rs:760+), after closing the planning ticket and creating children, we should check if any children are now unblocked:

```rust
async fn execute_planning_completion(...) -> Result<()> {
    // Step 1: Create worker types
    // Step 2: Create child tickets
    let created_ticket_ids = self.create_child_tickets_transactional(...).await?;

    // Step 3: Close planning ticket
    Ticket::close_ticket(&self.db, planning_ticket_id.as_str(), "planning_complete").await?;

    // Step 4: Check if planning ticket completion unblocks any children
    self.check_and_unblock_dependents(planning_ticket_id.as_str()).await?;

    // Step 5: Auto-enqueue ready child tickets
    self.enqueue_ready_child_tickets(&created_ticket_ids).await?;
}
```

### Fix 2: Ensure Initial Dependency Status is Correct

Check `create_child_tickets_transactional` to ensure tickets are created with correct initial status:
- Tickets with no dependencies → `dependency_status='ready'`
- Tickets with dependencies → `dependency_status='blocked'`

### Fix 3: Add Manual Dependency Resolution Tool

For cases where automatic resolution fails, add an MCP tool:

```rust
pub async fn resolve_ticket_dependencies(
    pool: &DbPool,
    queue_manager: Arc<QueueManager>,
    ticket_id: &str,
) -> Result<()> {
    // Force check and update dependency status
    let blocking_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM ticket_dependencies td
         INNER JOIN tickets dep ON td.parent_ticket_id = dep.ticket_id
         WHERE td.child_ticket_id = ?1 AND dep.state != 'closed'"
    )
    .bind(ticket_id)
    .fetch_one(pool).await?;

    if blocking_count == 0 {
        sqlx::query(
            "UPDATE tickets SET dependency_status = 'ready',
             updated_at = datetime('now') WHERE ticket_id = ?1"
        )
        .bind(ticket_id)
        .execute(pool).await?;

        // Resubmit if needed
        // ...
    }
}
```

## Testing Strategy

1. **Unit Test**: Create ticket with dependencies, close dependencies one by one, verify status updates
2. **Integration Test**: Planning ticket creates children with dependencies, verify all unblock correctly
3. **Race Condition Test**: Rapidly close multiple dependencies, verify dependent updates correctly

## Implementation Priority

1. **HIGH**: Fix 1 - Check dependencies after planning completion
2. **HIGH**: Fix 2 - Verify initial dependency status during creation
3. **MEDIUM**: Fix 3 - Add manual resolution tool for recovery
4. **LOW**: Add comprehensive logging to track dependency resolution flow

## Files to Modify

1. `src/workers/queue.rs` - Add dependency check after planning completion
2. `src/database/tickets.rs` - Verify initial dependency_status logic
3. `src/mcp/dependency_tools.rs` - Add manual resolution tool (optional)
