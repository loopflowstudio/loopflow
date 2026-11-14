#!/bin/zsh
# Run the built Swift test bundle without SwiftPM's .build lock. Args pass to the testing helper.
D=/Applications/Xcode.app/Contents/Developer; P=$D/Platforms/MacOSX.platform/Developer
B=${0:A:h:h:h}/swift/.build/debug/LoopflowSwiftPackageTests.xctest/Contents/MacOS/LoopflowSwiftPackageTests
cd ${0:A:h:h:h}/swift
exec env $(env | grep -E '^(LF_|REX_)' | cut -d= -f1 | sed 's/^/-u /') DYLD_FRAMEWORK_PATH=$P/Library/Frameworks:$P/Library/PrivateFrameworks DYLD_LIBRARY_PATH=$P/usr/lib \
  $D/Toolchains/XcodeDefault.xctoolchain/usr/libexec/swift/pm/swiftpm-testing-helper --test-bundle-path $B --testing-library swift-testing --parallel "$@" < /dev/null
