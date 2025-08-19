# Production Operations Guide

## Overview

This guide provides comprehensive operational procedures for the Vibe Ensemble MCP Server production deployment. It covers deployment, monitoring, troubleshooting, and maintenance procedures for enterprise-grade operations.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Production Deployment](#production-deployment)
3. [Monitoring and Alerting](#monitoring-and-alerting)
4. [Backup and Recovery](#backup-and-recovery)
5. [Security Operations](#security-operations)
6. [Troubleshooting](#troubleshooting)
7. [Maintenance Procedures](#maintenance-procedures)
8. [Incident Response](#incident-response)
9. [Performance Tuning](#performance-tuning)
10. [Compliance and Auditing](#compliance-and-auditing)

## Prerequisites

### Infrastructure Requirements

- **Kubernetes Cluster**: v1.25+ with RBAC enabled
- **Minimum Resources**: 16 CPU cores, 32GB RAM, 500GB storage
- **Network**: Load balancer with SSL termination
- **DNS**: Proper domain configuration with TLS certificates
- **Storage**: Fast SSD storage class for database and monitoring

### Access Requirements

- `kubectl` access to production cluster
- Docker registry access (GitHub Container Registry)
- Cloud storage access for backups (AWS S3/Google Cloud Storage)
- Monitoring dashboard access (Grafana)
- Log aggregation access (if configured)

### Tools Required

```bash
# Kubernetes management
kubectl >= 1.25.0
helm >= 3.8.0

# Container management
docker >= 20.10.0
buildx plugin

# Security scanning
trivy >= 0.35.0
hadolint >= 2.12.0

# Backup and restore
pg_dump/pg_restore >= 15.0
aws-cli >= 2.0 (for S3 backups)

# Monitoring
prometheus-client
grafana-cli
```

## Production Deployment

### Initial Deployment

1. **Prepare the environment**:
   ```bash
   # Create namespace
   kubectl create namespace vibe-ensemble
   
   # Label namespace for monitoring
   kubectl label namespace vibe-ensemble name=vibe-ensemble
   ```

2. **Generate secrets**:
   ```bash
   # Generate production secrets
   cd deployment/scripts
   ./generate-secrets.sh production
   
   # Apply secrets
   kubectl apply -f secrets-generated.yaml
   ```

3. **Deploy infrastructure components**:
   ```bash
   # Apply in order
   kubectl apply -f deployment/k8s/namespace.yaml
   kubectl apply -f deployment/k8s/configmap-production.yaml
   kubectl apply -f deployment/k8s/secrets.yaml
   kubectl apply -f deployment/k8s/postgres.yaml
   kubectl apply -f deployment/k8s/monitoring.yaml
   ```

4. **Deploy application**:
   ```bash
   # Wait for database to be ready
   kubectl wait --for=condition=ready pod -l app.kubernetes.io/name=postgres -n vibe-ensemble --timeout=300s
   
   # Apply application manifests
   kubectl apply -f deployment/k8s/deployment.yaml
   kubectl apply -f deployment/k8s/service.yaml
   kubectl apply -f deployment/k8s/ingress.yaml
   kubectl apply -f deployment/k8s/hpa.yaml
   kubectl apply -f deployment/k8s/pod-disruption-budget.yaml
   ```

5. **Configure networking and security**:
   ```bash
   # Apply security policies
   kubectl apply -f deployment/k8s/network-policy.yaml
   kubectl apply -f deployment/k8s/security-policies.yaml
   
   # Set up backup jobs
   kubectl apply -f deployment/k8s/backup-cronjob.yaml
   ```

### Deployment Verification

```bash
# Check all pods are running
kubectl get pods -n vibe-ensemble

# Verify services
kubectl get services -n vibe-ensemble

# Check ingress configuration
kubectl get ingress -n vibe-ensemble

# Test health endpoints
curl -f https://vibe-ensemble.example.com/api/health
curl -f https://vibe-ensemble.example.com/metrics
```

### Rolling Updates

```bash
# Update application image
kubectl set image deployment/vibe-ensemble-server \
  vibe-ensemble-server=ghcr.io/siy/vibe-ensemble-mcp:v1.2.0 \
  -n vibe-ensemble

# Monitor rollout
kubectl rollout status deployment/vibe-ensemble-server -n vibe-ensemble

# Rollback if needed
kubectl rollout undo deployment/vibe-ensemble-server -n vibe-ensemble
```

## Monitoring and Alerting

### Key Metrics to Monitor

1. **Application Metrics**:
   - HTTP request rate and latency
   - Error rates (4xx, 5xx)
   - Database connection pool usage
   - Memory and CPU utilization
   - WebSocket connection count

2. **Infrastructure Metrics**:
   - Pod restart count
   - Node resource utilization
   - Storage usage and IOPS
   - Network throughput

3. **Business Metrics**:
   - Agent registration rate
   - Message processing throughput
   - Knowledge repository growth
   - User session duration

### Grafana Dashboard Access

```bash
# Get Grafana admin password
kubectl get secret vibe-ensemble-secrets -n vibe-ensemble \
  -o jsonpath="{.data.grafana-admin-password}" | base64 -d

# Port forward to access Grafana
kubectl port-forward service/grafana-service 3000:3000 -n vibe-ensemble
```

Visit http://localhost:3000 and login with `admin` and the retrieved password.

### Alert Configuration

Critical alerts are configured in Prometheus and sent to Alertmanager. Key alerts include:

- **HighErrorRate**: HTTP error rate > 5%
- **HighLatency**: P99 latency > 1 second
- **PodCrashLooping**: Pod restarts > 5 in 1 hour
- **DatabaseConnectionFailure**: Database unavailable
- **HighMemoryUsage**: Memory usage > 85%
- **HighCPUUsage**: CPU usage > 80%

### Log Analysis

```bash
# View application logs
kubectl logs -f deployment/vibe-ensemble-server -n vibe-ensemble

# Search for errors
kubectl logs deployment/vibe-ensemble-server -n vibe-ensemble | grep ERROR

# View database logs
kubectl logs -f deployment/postgres -n vibe-ensemble
```

## Backup and Recovery

### Automated Backups

Backups are automated via CronJobs:
- **Daily backups**: Every day at 2 AM UTC
- **Weekly backups**: Every Sunday at 1 AM UTC with 90-day retention

### Manual Backup

```bash
# Create immediate backup
kubectl create job --from=cronjob/postgres-backup manual-backup-$(date +%Y%m%d) -n vibe-ensemble

# Monitor backup job
kubectl logs job/manual-backup-$(date +%Y%m%d) -n vibe-ensemble -f
```

### Database Restoration

```bash
# List available backups
aws s3 ls s3://vibe-ensemble-backups/postgresql/

# Restore from backup
cd deployment/scripts
./database-restore.sh --verify --force backup_file.sql.gz

# Verify restoration
kubectl exec -it deployment/postgres -n vibe-ensemble -- \
  psql -U vibe_ensemble -d vibe_ensemble -c "SELECT COUNT(*) FROM agents;"
```

### Disaster Recovery

1. **Complete Infrastructure Loss**:
   ```bash
   # Recreate cluster
   # Apply all manifests
   # Restore from latest backup
   ./database-restore.sh latest_backup.sql.gz
   ```

2. **Data Corruption**:
   ```bash
   # Stop application
   kubectl scale deployment vibe-ensemble-server --replicas=0 -n vibe-ensemble
   
   # Restore from known good backup
   ./database-restore.sh --verify backup_before_corruption.sql.gz
   
   # Restart application
   kubectl scale deployment vibe-ensemble-server --replicas=3 -n vibe-ensemble
   ```

## Security Operations

### Security Monitoring

1. **Container Security**:
   - Regular vulnerability scanning with Trivy
   - Runtime security monitoring with Falco
   - Admission control with OPA Gatekeeper

2. **Network Security**:
   - Network policies restricting pod communication
   - TLS encryption for all external communication
   - Regular certificate rotation

3. **Access Control**:
   - RBAC with minimal required permissions
   - Service account token rotation
   - Regular access review

### Security Incident Response

1. **Suspected Compromise**:
   ```bash
   # Isolate affected pods
   kubectl label pod SUSPECTED_POD quarantine=true -n vibe-ensemble
   
   # Collect forensic data
   kubectl logs SUSPECTED_POD -n vibe-ensemble > incident_logs.txt
   kubectl describe pod SUSPECTED_POD -n vibe-ensemble > incident_details.txt
   
   # Take memory dump if needed
   kubectl exec SUSPECTED_POD -n vibe-ensemble -- cat /proc/PID/maps
   ```

2. **Certificate Issues**:
   ```bash
   # Check certificate status
   kubectl describe certificate vibe-ensemble-tls-cert -n vibe-ensemble
   
   # Force certificate renewal
   kubectl delete secret vibe-ensemble-tls-cert -n vibe-ensemble
   kubectl annotate certificate vibe-ensemble-tls-cert force-renewal=true -n vibe-ensemble
   ```

### Regular Security Tasks

```bash
# Weekly vulnerability scan
trivy image ghcr.io/siy/vibe-ensemble-mcp:latest

# Monthly access review
kubectl get rolebindings -n vibe-ensemble
kubectl get clusterrolebindings | grep vibe-ensemble

# Quarterly security policy review
kubectl get networkpolicies -n vibe-ensemble
kubectl get podsecuritypolicies | grep vibe-ensemble
```

## Troubleshooting

### Common Issues

1. **Pod Startup Failures**:
   ```bash
   # Check pod status
   kubectl describe pod POD_NAME -n vibe-ensemble
   
   # Check resource constraints
   kubectl top pods -n vibe-ensemble
   
   # Check secrets and config
   kubectl get secrets,configmaps -n vibe-ensemble
   ```

2. **Database Connection Issues**:
   ```bash
   # Test database connectivity
   kubectl exec -it deployment/postgres -n vibe-ensemble -- \
     psql -U vibe_ensemble -d vibe_ensemble -c "SELECT 1;"
   
   # Check connection pool status
   kubectl exec -it deployment/postgres -n vibe-ensemble -- \
     psql -U vibe_ensemble -d vibe_ensemble -c "SELECT * FROM pg_stat_activity;"
   ```

3. **High Latency**:
   ```bash
   # Check resource utilization
   kubectl top pods -n vibe-ensemble
   kubectl top nodes
   
   # Analyze slow queries
   kubectl exec -it deployment/postgres -n vibe-ensemble -- \
     psql -U vibe_ensemble -d vibe_ensemble -c "SELECT query, mean_time, calls FROM pg_stat_statements ORDER BY mean_time DESC LIMIT 10;"
   ```

4. **Memory Issues**:
   ```bash
   # Check memory usage
   kubectl top pods -n vibe-ensemble --sort-by=memory
   
   # Check for memory leaks
   kubectl exec -it POD_NAME -n vibe-ensemble -- cat /proc/meminfo
   
   # Restart high-memory pods
   kubectl delete pod POD_NAME -n vibe-ensemble
   ```

### Performance Debugging

```bash
# Check application metrics
curl -s https://vibe-ensemble.example.com/metrics | grep http_request_duration

# Database performance
kubectl exec -it deployment/postgres -n vibe-ensemble -- \
  psql -U vibe_ensemble -d vibe_ensemble -c "SELECT * FROM pg_stat_user_tables;"

# Network latency testing
kubectl run netshoot --rm -i --tty --image nicolaka/netshoot -n vibe-ensemble
```

## Maintenance Procedures

### Scheduled Maintenance

1. **Monthly Updates**:
   ```bash
   # Update container images
   kubectl set image deployment/vibe-ensemble-server \
     vibe-ensemble-server=ghcr.io/siy/vibe-ensemble-mcp:latest -n vibe-ensemble
   
   # Update monitoring stack
   helm upgrade prometheus prometheus-community/kube-prometheus-stack
   ```

2. **Quarterly Tasks**:
   - Security patch review and application
   - Performance baseline review
   - Capacity planning assessment
   - Disaster recovery testing

3. **Annual Tasks**:
   - Full security audit
   - Infrastructure cost review
   - Technology stack evaluation

### Database Maintenance

```bash
# Weekly database maintenance
kubectl exec -it deployment/postgres -n vibe-ensemble -- \
  psql -U vibe_ensemble -d vibe_ensemble -c "VACUUM ANALYZE;"

# Monthly index maintenance
kubectl exec -it deployment/postgres -n vibe-ensemble -- \
  psql -U vibe_ensemble -d vibe_ensemble -c "REINDEX DATABASE vibe_ensemble;"

# Check database size
kubectl exec -it deployment/postgres -n vibe-ensemble -- \
  psql -U vibe_ensemble -d vibe_ensemble -c "SELECT pg_size_pretty(pg_database_size('vibe_ensemble'));"
```

## Incident Response

### Severity Levels

- **Critical (P0)**: Service completely unavailable
- **High (P1)**: Significant feature unavailable
- **Medium (P2)**: Performance degraded
- **Low (P3)**: Minor issues

### Response Procedures

1. **Critical Incidents**:
   - Immediate escalation to on-call engineer
   - Status page update within 5 minutes
   - Incident commander assignment
   - Regular status updates every 15 minutes

2. **High Priority Incidents**:
   - Acknowledgment within 15 minutes
   - Investigation start within 30 minutes
   - Status updates every 30 minutes

### Post-Incident Review

1. Create incident report with:
   - Timeline of events
   - Root cause analysis
   - Impact assessment
   - Action items for prevention

2. Update runbooks and procedures based on lessons learned

## Performance Tuning

### Application Performance

```bash
# Adjust JVM/runtime settings
kubectl patch deployment vibe-ensemble-server -n vibe-ensemble --patch '
spec:
  template:
    spec:
      containers:
      - name: vibe-ensemble-server
        env:
        - name: RUST_LOG
          value: "warn,vibe_ensemble=info"
'

# Scale horizontally
kubectl scale deployment vibe-ensemble-server --replicas=5 -n vibe-ensemble

# Update resource limits
kubectl patch deployment vibe-ensemble-server -n vibe-ensemble --patch '
spec:
  template:
    spec:
      containers:
      - name: vibe-ensemble-server
        resources:
          requests:
            memory: "1Gi"
            cpu: "500m"
          limits:
            memory: "4Gi"
            cpu: "2000m"
'
```

### Database Performance

```bash
# Tune PostgreSQL parameters
kubectl exec -it deployment/postgres -n vibe-ensemble -- \
  psql -U vibe_ensemble -d vibe_ensemble -c "ALTER SYSTEM SET shared_buffers = '1GB';"

kubectl exec -it deployment/postgres -n vibe-ensemble -- \
  psql -U vibe_ensemble -d vibe_ensemble -c "SELECT pg_reload_conf();"

# Add database indexes
kubectl exec -it deployment/postgres -n vibe-ensemble -- \
  psql -U vibe_ensemble -d vibe_ensemble -c "CREATE INDEX CONCURRENTLY idx_messages_timestamp ON messages(created_at);"
```

## Compliance and Auditing

### Audit Log Collection

```bash
# Kubernetes audit logs
kubectl logs -n kube-system kube-apiserver-NODE_NAME | grep audit

# Application audit logs
kubectl logs deployment/vibe-ensemble-server -n vibe-ensemble | grep AUDIT

# Database audit logs
kubectl exec -it deployment/postgres -n vibe-ensemble -- \
  tail -f /var/log/postgresql/postgresql.log
```

### Compliance Checks

```bash
# Security policy compliance
kubectl get networkpolicies -n vibe-ensemble
kubectl get podsecuritypolicies

# Resource usage compliance
kubectl top pods -n vibe-ensemble
kubectl describe resourcequota vibe-ensemble-quota -n vibe-ensemble

# Data retention compliance
# Check backup retention policies
aws s3 ls s3://vibe-ensemble-backups/postgresql/ | wc -l
```

### Regular Reports

Generate monthly reports including:
- Security incident summary
- Performance metrics
- Resource utilization
- Backup success rates
- Compliance status

## Emergency Contacts

- **On-call Engineer**: +1-XXX-XXX-XXXX
- **Infrastructure Team**: infrastructure@example.com
- **Security Team**: security@example.com
- **Database Admin**: dba@example.com

## Additional Resources

- [Kubernetes Operations Guide](https://kubernetes.io/docs/concepts/cluster-administration/)
- [PostgreSQL Administration](https://www.postgresql.org/docs/current/admin.html)
- [Prometheus Monitoring](https://prometheus.io/docs/practices/)
- [Security Best Practices](https://kubernetes.io/docs/concepts/security/)