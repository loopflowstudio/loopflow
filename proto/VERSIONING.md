# Protocol Versioning

Rules for maintaining compatibility across loopflow clients and servers.

## Version Format

Protocol versions follow semantic versioning: `MAJOR.MINOR.PATCH`

| Component | When to bump | Example |
|-----------|--------------|---------|
| **MAJOR** | Breaking changes | Remove field, change semantics |
| **MINOR** | Additive changes | New field, new method, new enum value |
| **PATCH** | Bug fixes | Fix typo in comment, clarify docs |

Current version: **1.0.0**

## Compatibility Rules

### Client → Server

```
┌─────────────────────────────────────────────────────────────────┐
│  Client checks server's protocol_version in GetHealth response  │
│                                                                 │
│  ┌────────────────────────────────────────────────────────┐    │
│  │ if server.major != client.major:                       │    │
│  │     REFUSE TO CONNECT                                  │    │
│  │                                                        │    │
│  │ if server.minor < client.minor:                        │    │
│  │     WARN: server may not support all features          │    │
│  │     proceed with caution                               │    │
│  │                                                        │    │
│  │ if server.minor > client.minor:                        │    │
│  │     OK: server has features client doesn't use         │    │
│  └────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

### Server → Client

Servers must:
1. Accept requests from clients with same major version
2. Ignore unknown fields (forward compatibility)
3. Return errors for unknown methods (not silently fail)

## Breaking Changes (MAJOR bump)

These require a major version bump:

- Removing a field
- Changing a field's type
- Changing a field's semantic meaning
- Removing an RPC method
- Changing an RPC method's signature
- Removing an enum value
- Changing an enum value's numeric assignment

## Additive Changes (MINOR bump)

These are safe additive changes:

- Adding a new optional field
- Adding a new RPC method
- Adding a new enum value (at the end)
- Adding a new message type
- Adding a new service

## Non-Breaking Changes (PATCH bump)

- Fixing comments or documentation
- Renaming a field (wire format unchanged)
- Changing default values (proto3 doesn't have explicit defaults anyway)

## Field Number Rules

**Reserved forever once used.** If you remove a field, reserve its number:

```protobuf
message Wave {
  reserved 7;  // was: deprecated_field
  reserved "deprecated_field";
}
```

**Never reuse field numbers.** Old data would be misinterpreted.

## Enum Value Rules

- Enum value 0 must be `UNSPECIFIED` (proto3 default)
- New values go at the end
- Never reuse enum value numbers
- Reserve removed values

## Event Schema Rules

Events are versioned with the protocol. Event payloads:

- Must include `event` field with type string
- Must include `timestamp` field
- New event types require MINOR bump
- Changing event payload requires MAJOR bump

## Migration Path

When making breaking changes:

1. Create new proto file in `v2/` directory
2. Implement new service alongside old
3. Deprecate old service with timeline
4. Remove old service after deprecation period

Example:
```
proto/loopflow/control/v1/control.proto  (deprecated)
proto/loopflow/control/v2/control.proto  (current)
```

## Compatibility Testing

Golden fixtures in `fixtures/` validate schema compatibility:

```bash
# Run compatibility tests
pytest tests/test_proto_fixtures.py

# Regenerate fixtures after intentional changes
python -m loopflow.proto.generate_fixtures
```

Tests verify:
1. All fixture files parse without error
2. Required fields are present
3. Enum values are valid
4. Event payloads match schema

## Implementation Checklist

When implementing a client:

- [ ] Check `protocol_version` on connection
- [ ] Refuse if major version differs
- [ ] Log warning if minor version is older
- [ ] Handle unknown fields gracefully
- [ ] Include `idempotency_key` for mutating operations
- [ ] Respect `retry_after_seconds` in errors

When implementing a server:

- [ ] Return `protocol_version` in GetHealth
- [ ] Ignore unknown fields in requests
- [ ] Return typed errors with `ErrorDetail`
- [ ] Track `idempotency_key` for deduplication
- [ ] Emit events with consistent timestamps
