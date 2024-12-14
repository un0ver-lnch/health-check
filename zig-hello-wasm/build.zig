const std = @import("std");
const Builder = std.build.Builder;
const builtin = @import("builtin");

pub fn build(b: *Builder) void {
    const target = b.standardTargetOptions(.{});
    const mode = b.standardReleaseOptions();

    const exe = b.addExecutable("example-true", "main.zig");
    exe.setTarget(target);
    exe.setBuildMode(mode);
    exe.install();
}
