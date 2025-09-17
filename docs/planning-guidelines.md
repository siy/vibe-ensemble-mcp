# Vibe-Ensemble Planning Guidelines

## Executive Summary

This document provides comprehensive guidelines for coordinators in the vibe-ensemble multi-agent system to create effective project plans, manage optimal task delegation, and oversee successful execution. These guidelines complement the task breakdown sizing methodology and provide practical frameworks for real-world project coordination.

## Core Planning Principles

### 1. Absolute Delegation Rule
**Coordinators coordinate, workers implement.** Never perform technical work yourself - all implementation must be delegated through tickets.

### 2. Context-Aware Optimization
Apply the task breakdown sizing methodology to balance performance gains (larger tasks) with reliability requirements (context limits).

### 3. Natural Boundary Respect
Align task breakdown with:
- Technology boundaries (different frameworks/languages)
- Functional boundaries (distinct business capabilities)
- Knowledge domain boundaries (different expertise requirements)
- Dependency isolation (minimal cross-task coupling)

### 4. Performance-First Planning
Optimize for:
- Minimum coordination overhead
- Maximum parallel execution opportunities
- Efficient resource utilization
- Reduced system complexity

## Project Planning Workflow

### Phase 1: Project Initialization

#### 1.1 Project Setup
```bash
# Create project with descriptive name and proper path
create_project(repository_name, local_path, description)
```

**Best Practices:**
- Use clear, descriptive repository names
- Ensure local path exists and is accessible
- Write concise but informative descriptions
- Include technology stack in description

#### 1.2 Planning Worker Verification
```bash
# Always check if planning worker exists first
list_worker_types(project_id)

# Create planning worker if missing
create_worker_type(project_id, "planning", planning_system_prompt, description)
```

**Critical Requirements:**
- Planning worker MUST include task breakdown sizing methodology
- System prompt must reference `task-breakdown-sizing.md`
- Include JSON output requirements for stage coordination
- Specify worker type creation capabilities

### Phase 2: Initial Ticket Creation

#### 2.1 Requirement Specification
Create comprehensive initial tickets with:
- Clear functional requirements
- Technical specifications
- Performance constraints
- Quality expectations
- Reference to sizing methodology

#### 2.2 Ticket Structure Template
```markdown
## Requirements
- [Functional requirements with clear acceptance criteria]

## Technical Specifications
- [Technology stack, frameworks, libraries]
- [Architecture patterns and constraints]
- [Performance and scalability requirements]

## Key Features
- [User-facing functionality]
- [System capabilities]
- [Integration requirements]

## Constraints
- [Performance, security, compatibility requirements]
- [Resource limitations]
- [Timeline considerations]

Reference: Use task breakdown sizing methodology from `task-breakdown-sizing.md`
```

### Phase 3: Execution Monitoring

#### 3.1 Event-Driven Coordination
Monitor system events in real-time and respond appropriately:

**Event Classification System:**
- **Informational**: Acknowledge only (project_created, worker_type_created)
- **Monitoring**: Brief oversight (ticket_created, ticket_claimed)
- **Intervention**: Active response (ticket_released, worker_stopped)
- **Completion**: Review outcomes (ticket_closed, ticket_stage_completed)

#### 3.2 Response Patterns
```bash
# For intervention events
get_ticket(ticket_id)  # Investigate current status
resume_ticket_processing(ticket_id, stage)  # Restart if stalled

# For completion events
get_ticket(ticket_id)  # Review outcomes
resolve_event(event_id, resolution_summary)  # Mark as handled
```

## Worker Type Creation Guidelines

### Planning Worker Template
The planning worker is the most critical component. Must include:

```markdown
## CORE RESPONSIBILITIES
1. Apply task breakdown sizing methodology from `task-breakdown-sizing.md`
2. Create optimal task breakdowns (120K token budget per task)
3. Create necessary worker types for each stage
4. Design efficient pipelines with 3-6 stages maximum

## CRITICAL CAPABILITIES
- Token estimation using provided framework
- Natural boundary recognition
- Worker type creation with optimized system prompts
- JSON output for stage coordination

## VALIDATION REQUIREMENTS
- All tasks under 120K token budget
- Clear dependency isolation
- Technology boundary respect
- Performance optimization focus
```

### Stage-Specific Worker Templates
Based on project analysis, create workers for:

**Common Stage Patterns:**
- **Foundation/Setup**: Project configuration, dependencies, core models
- **Backend/API**: Server implementation, database, business logic
- **Frontend/UI**: User interface, interactions, styling
- **Integration/Testing**: Quality assurance, deployment, documentation

### Worker System Prompt Best Practices

#### 1. Clear Role Definition
```markdown
You are a [STAGE] WORKER specializing in [technologies/domain].

## CORE RESPONSIBILITIES
- [Primary technical deliverables]
- [Quality standards and requirements]
- [Integration points with other stages]
```

#### 2. Technical Requirements
```markdown
## TECHNICAL REQUIREMENTS
- [Specific technologies and frameworks]
- [Coding standards and patterns]
- [Performance and quality criteria]
- [Testing and validation requirements]
```

#### 3. Dependencies and Integration
```markdown
## DEPENDENCIES
- [Required inputs from previous stages]
- [External dependencies and constraints]

## DELIVERABLES
- [Specific outputs and artifacts]
- [Documentation requirements]
- [Integration interfaces]
```

#### 4. JSON Output Schema
```markdown
## JSON OUTPUT REQUIREMENT
End your response with:
```json
{
  "ticket_id": "your_ticket_id",
  "outcome": "next_stage|prev_stage|coordinator_attention",
  "target_stage": "next_stage_name",
  "pipeline_update": ["stage1", "stage2", "stage3"],
  "comment": "Stage completion summary",
  "reason": "Rationale for progression"
}
```

## Task Scope Separation and Conflict Avoidance

### Critical Separation Principles

#### 1. File System Boundaries
Design tasks with clear file ownership to prevent conflicts:

**✅ Good Separation:**
- **Backend Task**: `src/main/java/`, `pom.xml`, `src/main/resources/application.properties`
- **Frontend Task**: `src/main/resources/static/`, `src/main/resources/templates/`
- **Testing Task**: `src/test/`, test configuration files

**❌ Poor Separation:**
- Multiple tasks editing the same configuration files
- Overlapping directory responsibilities
- Shared utility files without clear ownership

#### 2. Technology Stack Isolation
Separate tasks by technology concerns:

**Backend Isolation:**
- Database schemas and migrations
- API endpoint definitions
- Business logic implementation
- Server configuration

**Frontend Isolation:**
- UI components and styling
- Client-side interactions
- Asset management
- Browser-specific concerns

**Integration Isolation:**
- End-to-end testing
- Deployment scripts
- Performance testing
- Documentation

#### 3. Interface Contract Definition
Establish clear contracts between tasks:

```markdown
## API Contract Example
- **Backend Provides**: REST endpoints at `/api/todos` with JSON responses
- **Frontend Consumes**: Standard HTTP methods (GET, POST, PUT, DELETE)
- **Data Format**: Agreed JSON schema for Todo objects
- **Error Handling**: Standard HTTP status codes and error responses
```

#### 4. Dependency Direction Management
Ensure unidirectional dependencies:

```
Planning → Backend → Frontend → Integration
     ↓         ↓         ↓
   Contracts  APIs    Testing
```

**Rules:**
- Later stages can depend on earlier stages
- Earlier stages NEVER depend on later stages
- All dependencies must be explicit and documented

#### 5. Resource Allocation Boundaries
Prevent resource conflicts:

**Port Allocation:**
- Development server: 8080 (backend task)
- Asset serving: 8081 (frontend task, if needed)
- Testing server: 8082 (integration task)

**Database/Storage:**
- Production schemas: backend task
- Test data: integration task
- Mock data: frontend task (if needed)

### Conflict Prevention Strategies

#### 1. Scope Definition Templates
For each task, explicitly define:

```markdown
## TASK SCOPE BOUNDARIES

### Owns (Full Control):
- [List of files, directories, configurations this task controls]

### Reads (Reference Only):
- [List of files this task can read but not modify]

### Provides (Interface):
- [APIs, contracts, outputs this task delivers to other tasks]

### Requires (Dependencies):
- [Inputs, APIs, contracts this task needs from other tasks]

### Never Touches:
- [Explicitly forbidden files, directories, configurations]
```

#### 2. Sequential vs Parallel Execution Planning
Design tasks for safe parallelization:

**Sequential Tasks (Must be ordered):**
1. Planning → Backend (needs architecture)
2. Backend → Frontend (needs API contracts)
3. Frontend → Integration (needs complete implementation)

**Parallel Tasks (Can run simultaneously):**
- Documentation and Testing preparation
- Different microservices in distributed systems
- Independent feature branches

#### 3. Change Impact Analysis
Before creating tasks, analyze potential conflicts:

**High-Risk Overlap Areas:**
- Configuration files (pom.xml, package.json, etc.)
- Shared utilities or common libraries
- Database migration scripts
- Deployment configurations

**Mitigation Strategies:**
- Assign shared files to single task
- Create configuration templates in planning stage
- Use feature flags for gradual integration
- Implement merge conflict resolution procedures

#### 4. Workspace Organization
Structure project layout for clear separation:

```
project/
├── backend/           # Backend task owns this
│   ├── src/main/java/
│   ├── src/main/resources/
│   └── pom.xml
├── frontend/          # Frontend task owns this
│   ├── static/
│   ├── templates/
│   └── assets/
├── tests/            # Integration task owns this
│   ├── integration/
│   ├── e2e/
│   └── performance/
├── docs/             # Shared (read-only for most tasks)
└── scripts/          # Deployment task owns this
```

### Conflict Resolution Procedures

#### 1. Detection Mechanisms
Monitor for potential conflicts:
- File modification overlaps
- API contract changes
- Dependency version conflicts
- Resource allocation collisions

#### 2. Resolution Strategies
When conflicts occur:

**Minor Conflicts:**
- Coordinator mediates through event resolution
- Tasks negotiate through JSON outputs
- Merge strategies applied automatically

**Major Conflicts:**
- Pause conflicting tasks
- Coordinator creates resolution ticket
- Restart with clarified boundaries

#### 3. Prevention Validation
Include in planning checklist:
- [ ] No file system overlaps between tasks
- [ ] Clear interface contracts defined
- [ ] Dependencies flow in single direction
- [ ] Resource allocation documented
- [ ] Conflict resolution procedures established

## Quality Assurance Framework

### Planning Quality Checklist
- [ ] Task breakdown methodology applied correctly
- [ ] All tasks under 120K token budget with safety buffers
- [ ] Natural boundaries respected
- [ ] Dependencies properly isolated
- [ ] **Scope boundaries clearly defined to prevent conflicts**
- [ ] **Interface contracts established between tasks**
- [ ] **File system ownership documented**
- [ ] Execution order optimized for performance
- [ ] Worker types created for all stages
- [ ] JSON output requirements specified

### Execution Quality Checklist
- [ ] Workers spawning automatically for new stages
- [ ] Stage transitions progressing smoothly
- [ ] No tickets stuck without worker assignment
- [ ] Event responses appropriate to event classification
- [ ] Performance targets being met
- [ ] Quality standards maintained

### Intervention Criteria
Take action when:
- Tickets remain in same stage >30 minutes without activity
- Worker spawning failures occur
- Stage transitions fail repeatedly
- Quality deliverables don't meet standards
- Performance requirements not achieved

## Real-World Examples

### Example 1: Todo Application Success Pattern

**Project Scope:** Minimalistic todo app with HTMX + Java 21/Vert.x
**Total Complexity:** 140K tokens, medium complexity

#### Optimal Breakdown Applied:
1. **Planning** (Planning worker) → ✅ Completed
   - Applied sizing methodology correctly
   - Created 3 specialized worker types
   - Designed 4-stage pipeline

2. **Backend Development** (40-50K tokens) → ✅ Completed
   - Java 21 + Vert.x implementation
   - In-memory storage with capacity limits
   - REST API and HTMX endpoints

3. **Frontend Development** (30-50K tokens) → 🔄 In Progress
   - HTMX + Pico CSS implementation
   - Responsive design
   - Dynamic interactions

4. **Integration** (20-30K tokens) → ⏳ Pending
   - End-to-end testing
   - Performance validation
   - Deployment preparation

#### Key Success Factors:
- ✅ Sizing methodology applied correctly
- ✅ Natural technology boundaries respected
- ✅ Token budgets within safe limits
- ✅ Clear dependency isolation
- ✅ Automatic stage progression

### Example 2: Complex Web Application Pattern

**Project Scope:** E-commerce platform with React + Node.js + PostgreSQL
**Total Complexity:** 400K tokens, high complexity

#### Recommended Breakdown:
1. **Planning & Architecture** (Planning worker)
2. **Database & Backend Core** (80K tokens)
3. **API Layer & Authentication** (90K tokens)
4. **Frontend Core Components** (85K tokens)
5. **Advanced Features & Integration** (75K tokens)
6. **Testing & Deployment** (70K tokens)

#### Adaptation Strategies:
- **Increased Granularity**: Higher complexity requires more focused tasks
- **Technology Isolation**: Separate database, backend, and frontend concerns
- **Security Focus**: Dedicated authentication and security considerations
- **Advanced Features**: Complex business logic gets dedicated attention

## Troubleshooting Common Issues

### Issue: Planning Worker Creates Oversized Tasks
**Symptoms:** Token estimates >120K, context warnings
**Solutions:**
- Review planning worker system prompt
- Emphasize sizing methodology application
- Add explicit token budget enforcement
- Include validation checkpoint requirements

### Issue: Too Many Small Tasks
**Symptoms:** >8 stages, excessive coordination overhead
**Solutions:**
- Merge compatible technology tasks
- Combine setup with initial implementation
- Review natural boundary identification
- Emphasize performance optimization focus

### Issue: Workers Not Spawning
**Symptoms:** Tickets stuck in stages, no automatic progression
**Solutions:**
- Verify worker types exist for all stages
- Check JSON output format in worker prompts
- Use `resume_ticket_processing()` to restart
- Monitor events for spawning failures

### Issue: Stage Transitions Failing
**Symptoms:** Workers complete but don't advance pipeline
**Solutions:**
- Verify JSON output schema in worker prompts
- Check `target_stage` values match existing worker types
- Ensure `pipeline_update` arrays are correct
- Review stage naming consistency

### Issue: Quality Standards Not Met
**Symptoms:** Deliverables incomplete, requirements missed
**Solutions:**
- Enhance worker system prompts with specific requirements
- Add quality validation steps to stages
- Include acceptance criteria in initial tickets
- Implement review checkpoints

## Performance Optimization Strategies

### Parallel Execution Optimization
- Design independent stages that can run concurrently
- Minimize sequential dependencies
- Create clear interface contracts between stages
- Plan for pipeline branching when beneficial

### Resource Efficiency
- Balance task sizes for optimal resource utilization
- Avoid creating unnecessary worker types
- Reuse compatible worker types across similar projects
- Monitor and optimize worker spawn patterns

### Coordination Minimization
- Design self-contained tasks with clear boundaries
- Minimize cross-task communication requirements
- Use standardized interfaces between stages
- Automate progression decisions through JSON outputs

## Advanced Planning Patterns

### Multi-Project Coordination
When managing multiple related projects:
- Create shared worker types for common technologies
- Establish consistent naming conventions
- Design reusable planning templates
- Coordinate resource allocation across projects

### Iterative Development Planning
For projects requiring iterative development:
- Plan for multiple development cycles
- Design flexible pipeline structures
- Include feedback incorporation stages
- Plan for requirement evolution

### Emergency Response Planning
For urgent fixes or critical issues:
- Create expedited planning processes
- Design minimal viable task breakdowns
- Establish escalation procedures
- Plan for rapid deployment capabilities

## Continuous Improvement

### Metrics Collection
Track and analyze:
- Task completion times by stage type
- Context usage patterns across workers
- Quality outcomes and rework rates
- Coordination overhead measurements

### Process Refinement
Regularly review and update:
- Worker type templates based on outcomes
- Task sizing guidelines based on performance data
- Planning processes based on project learnings
- Quality standards based on results

### Knowledge Management
Maintain and evolve:
- Best practice documentation
- Common pattern libraries
- Troubleshooting guides
- Success story case studies

## Conclusion

Effective planning in vibe-ensemble requires balancing multiple considerations: performance optimization, quality assurance, resource efficiency, and system reliability. By following these guidelines and consistently applying the task breakdown sizing methodology, coordinators can achieve optimal project outcomes while maintaining system performance and reliability.

The key to success lies in treating coordination as a specialized skill that requires systematic approaches, continuous monitoring, and adaptive responses to changing conditions. These guidelines provide the framework for developing that expertise and achieving consistent success across diverse project types.

Remember: **Great coordination is invisible to users but essential for system success.** Focus on creating conditions for worker success rather than trying to do the work yourself.