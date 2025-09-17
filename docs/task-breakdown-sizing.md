# Task Breakdown Sizing Methodology

## Executive Summary

This methodology provides a systematic approach for agents to perform optimal task breakdown that maximizes performance while avoiding conversation compaction during execution. The method balances two competing goals:

1. **Performance Optimization**: Larger tasks reduce coordination overhead and total token usage
2. **Reliability Assurance**: Tasks must stay within context windows to prevent compaction

The methodology produces task granularity that aligns with seasoned developer intuition while providing mathematical precision for consistent application across different project types.

## Core Principles

### The Context-Performance Trade-off

**Large Tasks Benefits:**
- Fewer worker spawns and context switches
- Reduced coordination overhead between stages
- Lower cumulative token usage across the system
- More efficient batch processing of related operations

**Small Tasks Benefits:**
- Workers remain within effective context limits
- No mid-task memory management interruptions
- Cleaner handoffs between execution stages
- Higher resilience to execution interruptions

### Optimal Size Formula

```
Optimal Task Size = MAX(Task Complexity)
WHERE Context Usage < (Effective Context Limit - Safety Buffer)
AND Dependencies are properly isolated
```

## Context Window Budget Framework

### Effective Context Calculation

```
Effective Context = Total Context Window × Utilization Factor
Where:
- Total Context Window = ~200K tokens (typical modern LLM)
- Utilization Factor = 0.75 (accounting for system overhead)
- Effective Context = ~150K tokens available for task execution
```

### Safety Buffer Allocation

```
Task Budget = Effective Context - Safety Buffer
Where:
- Safety Buffer = 30K tokens (20% of effective context)
- Task Budget = ~120K tokens maximum per task
```

## Token Estimation Framework

### Base Token Costs by Operation Type

#### File Generation Costs
- **Simple Configuration Files**: 200-500 tokens per file
- **Basic Code Files**: 800-1,500 tokens per file
- **Complex Implementation Files**: 2,000-5,000 tokens per file
- **Documentation Files**: 1,000-3,000 tokens per file

#### Context Reading Costs
- **Documentation Reading**: 5,000-15,000 tokens per technology
- **Example Code Analysis**: 3,000-8,000 tokens per reference
- **Library/Framework Research**: 8,000-20,000 tokens per major dependency

#### Iteration and Debugging Costs
- **Initial Implementation**: 1.0× base cost
- **Refinement Iterations**: +0.3× per expected iteration
- **Error Resolution**: +0.5× for complex integrations

### Project Type Multipliers

#### Web Applications
- **Frontend-Heavy**: 1.2× multiplier (styling iterations)
- **Backend-Heavy**: 1.1× multiplier (business logic complexity)
- **Full-Stack**: 1.3× multiplier (integration complexity)

#### System Applications
- **CLI Tools**: 0.9× multiplier (simpler interactions)
- **Services/APIs**: 1.1× multiplier (configuration complexity)
- **Libraries**: 0.8× multiplier (focused scope)

## Step-by-Step Task Breakdown Algorithm

### Phase 1: Scope Analysis
1. **Identify Major Components**: List all high-level system components
2. **Map Dependencies**: Create dependency graph between components
3. **Estimate Complexity**: Assign complexity scores to each component
4. **Calculate Base Token Requirements**: Use estimation framework

### Phase 2: Natural Boundaries
1. **Technology Boundaries**: Group by similar tech stacks/frameworks
2. **Functional Boundaries**: Group by business/functional cohesion
3. **Dependency Isolation**: Ensure minimal cross-task dependencies
4. **Knowledge Domain Boundaries**: Group by required expertise areas

### Phase 3: Size Optimization
1. **Initial Grouping**: Combine components within natural boundaries
2. **Token Budget Check**: Verify each group stays within task budget
3. **Split Oversized Tasks**: Break down tasks exceeding budget
4. **Merge Undersized Tasks**: Combine small tasks for efficiency

### Phase 4: Validation
1. **Dependency Verification**: Confirm minimal cross-task interference
2. **Context Safety Check**: Ensure comfortable margin below limits
3. **Performance Assessment**: Validate reasonable task count
4. **Execution Order Planning**: Establish optimal execution sequence

## Practical Application Examples

### Example 1: Todo Application (Java + HTMX)

#### Scope Analysis
- **Total Estimated Tokens**: ~140K tokens
- **Major Components**: Project setup, backend core, REST API, frontend, testing
- **Complexity Assessment**: Medium complexity, moderate iterations expected

#### Natural Boundaries
1. **Setup + Core Models**: Configuration, data models, core services
2. **REST API + Verticle**: HTTP handling, routing, async processing
3. **Frontend + HTMX**: UI, styling, dynamic interactions
4. **Testing + Deployment**: Quality assurance, documentation

#### Token Estimation
1. **Setup + Core Models**: 30-40K tokens
   - Maven configuration: 2K
   - Data models: 5K
   - Core service implementation: 15K
   - Documentation reading: 10K
   - Refinement buffer: 8K

2. **REST API + Verticle**: 40-50K tokens
   - Vert.x verticle: 10K
   - REST endpoints: 15K
   - JSON handling: 8K
   - Documentation reading: 12K
   - Integration testing: 10K

3. **Frontend + HTMX**: 30-50K tokens
   - HTML structure: 8K
   - CSS styling: 12K
   - HTMX interactions: 15K
   - Responsive design: 10K
   - Cross-browser testing: 8K

4. **Testing + Deployment**: 20-30K tokens
   - Unit tests: 12K
   - Integration tests: 8K
   - Documentation: 6K
   - Deployment scripts: 4K

#### Validation Results
- ✅ All tasks under 120K token budget
- ✅ Clear dependency isolation
- ✅ Logical execution order
- ✅ Balanced workload distribution

### Example 2: Microservice API (Spring Boot)

#### Adjusted Breakdown for Higher Complexity
1. **Project Setup + Configuration**: 35K tokens
2. **Data Layer + Models**: 45K tokens
3. **Business Logic + Services**: 55K tokens
4. **REST Controllers + Security**: 50K tokens
5. **Testing + Documentation**: 40K tokens

#### Key Adjustments
- **Increased Granularity**: Higher complexity requires more focused tasks
- **Security Isolation**: Separate task for security concerns
- **Data Layer Focus**: Complex data operations get dedicated task

## Decision Trees and Guidelines

### When to Split Tasks Further

```
IF (Estimated Tokens > 100K) THEN
    Split along largest natural boundary
ELSIF (>3 Major Technologies in Single Task) THEN
    Split by technology boundaries
ELSIF (>5 Complex Files Expected) THEN
    Split by functional boundaries
ELSE
    Task size is appropriate
```

### When to Merge Tasks

```
IF (Estimated Tokens < 20K) AND (Compatible Technology) THEN
    Consider merging with adjacent task
ELSIF (Minimal Dependency Coupling) AND (Combined < 80K) THEN
    Merge for efficiency
ELSE
    Keep separate for clarity
```

### Quality Checkpoints

#### Pre-Execution Validation
- [ ] Each task has clear, measurable deliverables
- [ ] Token estimates include safety buffers
- [ ] Dependencies are explicitly documented
- [ ] Execution order is optimized

#### Post-Breakdown Review
- [ ] Total task count is reasonable (3-7 tasks for most projects)
- [ ] No single task dominates the project scope
- [ ] All major system components are covered
- [ ] Integration points are clearly defined

## Troubleshooting Common Issues

### Issue: Tasks Too Large
**Symptoms**: Estimated tokens >120K, complex integration requirements
**Solutions**:
- Split along technology boundaries
- Separate configuration from implementation
- Isolate complex algorithms or business logic

### Issue: Tasks Too Small
**Symptoms**: Many tasks <20K tokens, excessive coordination overhead
**Solutions**:
- Merge compatible technology tasks
- Combine setup with initial implementation
- Group related configuration tasks

### Issue: Unclear Dependencies
**Symptoms**: Circular dependencies, unclear execution order
**Solutions**:
- Create explicit interface definition tasks
- Separate shared utilities into dedicated tasks
- Establish clear API contracts between tasks

## Adaptation Guidelines

### Project Size Scaling
- **Small Projects (<50K total tokens)**: 2-3 tasks maximum
- **Medium Projects (50-200K tokens)**: 3-5 tasks optimal
- **Large Projects (200K+ tokens)**: 5-8 tasks, consider sub-project breakdown

### Team Considerations
- **Single Developer**: Prefer larger tasks for context continuity
- **Multiple Developers**: Smaller tasks for parallel execution
- **Mixed Experience**: Balance complex and simple tasks

### Technology Familiarity
- **Familiar Tech Stack**: Larger tasks acceptable
- **New Technologies**: Smaller tasks for learning curve
- **Bleeding Edge**: Conservative sizing with extra buffers

## Conclusion

This methodology provides a systematic, repeatable approach to task breakdown that optimizes for both performance and reliability. By following these guidelines, agents can consistently produce task structures that:

1. Maximize system efficiency through optimal task sizing
2. Minimize execution risks through proper context management
3. Align with developer intuition through natural boundary recognition
4. Scale appropriately across different project types and complexities

The key to success is balancing mathematical precision with practical engineering judgment, ensuring that the resulting task breakdown serves both the system's technical constraints and the project's development goals.