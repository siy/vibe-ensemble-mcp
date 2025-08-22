# Security Review Checklist for {{project_name}}

When performing security-focused reviews, ensure you check for these common issues:

## Input Validation & Sanitization
- [ ] All user inputs are validated at entry points
- [ ] Input size limits are enforced appropriately
- [ ] Special characters are handled safely
- [ ] File upload restrictions are in place
- [ ] URL/path inputs are sanitized to prevent traversal attacks

## Authentication & Authorization  
- [ ] Authentication mechanisms are implemented correctly
- [ ] Session management follows security best practices
- [ ] Authorization checks are performed at appropriate levels
- [ ] Privileged operations require proper permissions
- [ ] Default credentials are not used

## Data Protection
- [ ] Sensitive data is encrypted at rest and in transit
- [ ] Cryptographic keys are managed securely
- [ ] Personal data handling complies with privacy regulations
- [ ] Logging does not expose sensitive information
- [ ] Database queries use parameterized statements

{{#if (eq primary_language "rust")}}
## Rust-Specific Security Checks
- [ ] Unsafe blocks are justified and properly documented
- [ ] External dependencies are from trusted sources
- [ ] Serialization/deserialization is safe from attacks
- [ ] Memory management avoids buffer overflows
- [ ] Concurrent code prevents race conditions
{{/if}}

{{#if (eq primary_language "python")}}
## Python-Specific Security Checks
- [ ] `eval()` and `exec()` are avoided or properly sandboxed
- [ ] Pickle deserialization is avoided with untrusted data
- [ ] SQL injection prevention with parameterized queries
- [ ] Template injection prevention in web frameworks
- [ ] Proper exception handling to avoid information disclosure
{{/if}}

## Error Handling & Logging
- [ ] Error messages don't reveal system information
- [ ] Failures are logged appropriately for monitoring
- [ ] Stack traces are not exposed to end users
- [ ] Rate limiting is implemented for sensitive operations
- [ ] Security events are properly audited

## Dependencies & Supply Chain
- [ ] All dependencies are up-to-date and maintained
- [ ] Known vulnerabilities in dependencies are addressed
- [ ] Dependency sources are trusted and verified
- [ ] License compliance is maintained
- [ ] Regular security audits of dependencies are performed

Remember: Security is not a one-time check but an ongoing concern that should be integrated into the development process.