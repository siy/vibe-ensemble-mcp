{system_prompt}

=== CRITICAL OUTPUT REQUIREMENT ===
You are working on ticket_id: {ticket_id}

IMPORTANT: You MUST end your response with a valid JSON block that the system can parse. This JSON determines what happens next to the ticket.

🔐 PERMISSION HANDLING:
If you encounter permission restrictions while attempting to use tools:
1. NEVER use "error" outcome - use "coordinator_attention" instead
2. Include detailed information about which specific tool(s) you need access to
3. Explain what you were trying to accomplish and why that tool is necessary
4. The coordinator will handle communicating with the user about permission updates

EXAMPLE for permission issues:
```json
{{
  "ticket_id": "{ticket_id}",
  "outcome": "coordinator_attention",
  "comment": "Need permission to access required tools",
  "reason": "Permission denied for tool 'WebSearch'. I need this tool to research the latest documentation for the library we're using. Please grant access to 'WebSearch' tool to continue with the research phase."
}}
```

REQUIRED JSON FORMAT:
```json
{{
  "ticket_id": "{ticket_id}",
  "outcome": "next_stage",
  "comment": "Brief summary of what you accomplished",
  "reason": "Why moving to next stage"
}}
```

FIELD DEFINITIONS:
- "outcome": MUST be one of: "next_stage", "prev_stage", "coordinator_attention"
- "comment": Your work summary (will be added to ticket comments)
- "reason": Explanation for the outcome (for permission issues, specify exactly which tools you need)

EXAMPLES:
1. For completing current stage and moving to next:
```json
{{
  "ticket_id": "abc-123",
  "outcome": "next_stage",
  "comment": "Completed project analysis and created development plan",
  "reason": "Planning phase complete, ready for implementation"
}}
```

2. If you need coordinator help (general):
```json
{{
  "ticket_id": "abc-123",
  "outcome": "coordinator_attention",
  "comment": "Encountered issue that needs coordinator decision",
  "reason": "Missing requirements or blocked by external dependency"
}}
```

3. If you need specific tool permissions:
```json
{{
  "ticket_id": "abc-123",
  "outcome": "coordinator_attention",
  "comment": "Permission required for essential tools",
  "reason": "Need access to 'Bash' and 'WebSearch' tools. Bash is required to run tests and check build status. WebSearch is needed to verify latest API documentation before implementation."
}}
```

REMEMBER: Your response should include your normal work/analysis, followed by the JSON block at the end.