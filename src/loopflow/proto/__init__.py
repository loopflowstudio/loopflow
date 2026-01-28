"""Generated protobuf and gRPC bindings for loopflow protocol.

This package contains auto-generated code from proto files.
Do not edit directly - regenerate with `python -m loopflow.proto.generate`.

Structure:
    loopflow/control/v1/
        control_pb2.py       Control plane messages
        control_pb2_grpc.py  Control plane service stubs
    loopflow/engine/v1/
        engine_pb2.py        Engine contract messages
        engine_pb2_grpc.py   Engine contract service stubs

Usage:
    from loopflow.proto.loopflow.control.v1 import control_pb2, control_pb2_grpc
    from loopflow.proto.loopflow.engine.v1 import engine_pb2, engine_pb2_grpc

    # Create messages
    pv = control_pb2.ProtocolVersion(major=1, minor=0, patch=0)
    health = control_pb2.GetHealthResponse(version="0.7.0", protocol_version=pv)

See proto/README.md for schema documentation and regeneration instructions.
"""
