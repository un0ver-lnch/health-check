const std = @import("std");
const builtin = @import("builtin");

const targets: []const std.Target.Query = &.{ .{ .cpu_arch = .aarch64, .os_tag = .linux, .abi = .gnu }, .{ .cpu_arch = builtin.cpu.arch, .os_tag = builtin.os.tag, .abi = .gnu } };

const app_name = "bitsearch-health";

pub fn build(b: *std.Build) !void {
    for (targets) |target| {
        if (builtin.cpu.arch == target.cpu_arch) {
            const run_in_release_mode_option = b.option(bool, "release", "Run the library in release mode") orelse false;

            const exe = b.addExecutable(.{
                .name = app_name,
                .root_source_file = b.path("src/main.zig"),
                .target = b.resolveTargetQuery(target),
                .optimize = if (run_in_release_mode_option) .ReleaseFast else .Debug,
            });
            exe.linkLibC();
            const target_output = b.addInstallArtifact(exe, .{
                .dest_dir = .{
                    .override = .{
                        .custom = try target.zigTriple(b.allocator),
                    },
                },
            });

            const exe_check = b.addExecutable(.{
                .name = "foo",
                .root_source_file = b.path("src/main.zig"),
                .target = b.resolveTargetQuery(target),
                .optimize = .Debug,
            });
            exe_check.linkLibC();

            // These two lines you might want to copy
            // (make sure to rename 'exe_check')
            const check = b.step("check", "Check if foo compiles");
            check.dependOn(&exe_check.step);

            const run_exe = b.addRunArtifact(exe);
            const run_step = b.step("run", "Run the application");
            run_step.dependOn(&run_exe.step);
            b.getInstallStep().dependOn(&target_output.step);
        }
        const lib = b.addSharedLibrary(.{
            .name = app_name,
            .root_source_file = b.path("src/main.zig"),
            .target = b.resolveTargetQuery(target),
            .optimize = .ReleaseFast,
        });

        lib.linkLibC();

        const so_module_install = b.addInstallArtifact(lib, .{
            .dest_dir = .{
                .override = .{
                    .custom = try target.zigTriple(b.allocator),
                },
            },
        });

        b.getInstallStep().dependOn(&so_module_install.step);

        if (builtin.cpu.arch == target.cpu_arch) {
            const system_command_route = try std.fmt.allocPrint(b.allocator, "./zig-out/{s}/{s}", .{ try target.zigTriple(b.allocator), so_module_install.dest_sub_path });

            const copy_lib = b.addSystemCommand(&[_][]const u8{ "cp", system_command_route, "../modules/" });
            copy_lib.step.dependOn(&so_module_install.step);

            const copy_lib_step = b.step("copy", "Copy the shared library");
            copy_lib_step.dependOn(&copy_lib.step);
        }
    }
}
