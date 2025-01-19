const std = @import("std");
const Allocator = std.mem.Allocator;
const EnvMap = std.process.EnvMap;

// Not the full response but we ignore the rest
const WeatherResponse = struct {
    sys: struct {
        country: []const u8,
        sunrise: u64,
        sunset: u64,
    },
    clouds: struct {
        all: u32,
    },
    visibility: u32,
};

pub fn main() !void {
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena.deinit();
    const allocator = arena.allocator();
    var envmap = try std.process.getEnvMap(allocator);
    defer envmap.deinit();
    const result = try process_weather(allocator, envmap);
    std.debug.print("Result: {s}\n", .{result});
}

export fn free_string(s: [*:0]const u8) void {
    const c_allocator = std.heap.c_allocator;
    const lenght = std.mem.len(s);

    const value = s[0..lenght];

    c_allocator.free(value);
}

export fn start(envvars_string: [*:0]const u8) [*]const u8 {
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena.deinit();
    const allocator = arena.allocator();

    const full_envvar_string = std.fmt.allocPrint(allocator, "{s}", .{envvars_string}) catch {
        return "KV:light###0";
    };

    var envmap = std.process.getEnvMap(allocator) catch {
        return "KV:light###0";
    };
    defer envmap.deinit();

    var envvars_string_iter = std.mem.split(u8, full_envvar_string, ";;;");

    while (envvars_string_iter.next()) |envvar| {
        var envvar_iter = std.mem.split(u8, envvar, "=");
        const key = envvar_iter.next() orelse "";
        const value = envvar_iter.next() orelse "";

        if (std.mem.eql(u8, key, "") and std.mem.eql(u8, value, "")) {
            continue;
        }

        envmap.put(key, value) catch unreachable;
    }

    const result = process_weather(allocator, envmap) catch {
        return "KV:light###0";
    };

    // Dont free this memory as we are returning it to the caller
    const c_allocator = std.heap.c_allocator;

    var result_c_string: [*:0]u8 = undefined;
    result_c_string = c_allocator.allocSentinel(u8, result.len + 1, 0) catch {
        return "KV:light###0";
    };
    errdefer c_allocator.free(result_c_string);
    @memcpy(result_c_string[0..result.len], result[0..]);
    result_c_string[result.len] = 0;
    return result_c_string;
}

fn process_weather(allocator: Allocator, envmap: EnvMap) ![]const u8 {
    const key = envmap.get("OPENWEATHER_KEY") orelse "";

    if (std.mem.eql(u8, key, "")) {
        std.debug.print("You need to set the OPENWEATHER_KEY variable", .{});
        return error.EnvVarNotSet;
    }

    var client = std.http.Client{ .allocator = allocator };
    defer std.http.Client.deinit(&client);

    const uri = try std.Uri.parse(try std.fmt.allocPrint(allocator, "https://api.openweathermap.org/data/2.5/weather?lat={}&lon={}&appid={s}", .{ 40.416775, -3.703790, key }));

    var header_buffer = [_]u8{0} ** 2048;

    var result = try std.http.Client.open(
        &client,
        .GET,
        uri,
        .{
            .server_header_buffer = &header_buffer,
        },
    );
    defer result.deinit();

    try result.send();

    try result.wait();

    var reader = result.reader();

    const conents = try reader.readAllAlloc(allocator, 1_000);

    const parsed_contents = try std.json.parseFromSliceLeaky(WeatherResponse, allocator, conents, .{ .ignore_unknown_fields = true });

    // Get the current time and compare
    const current_time = @divFloor(std.time.milliTimestamp(), 1000);

    const offset = 60 * 60; // offset by 1 hour
    const sunrise = parsed_contents.sys.sunrise + offset;
    const sunset = parsed_contents.sys.sunset - offset;

    if (current_time >= sunrise and current_time <= sunset) {
        // DAYTIME
        // TODO: detect bad wheater or bad visibility to turn on the light
        return "KV:light###0";
    } else {
        // NIGHTTIME
        return "KV:light###1";
    }
}
