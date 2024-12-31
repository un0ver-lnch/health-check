const std = @import("std");

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

    const key = envmap.get("OPENWEATHER_KEY") orelse "";

    if (std.mem.eql(u8, key, "")) {
        std.debug.print("You need to set the OPENWEATHER_KEY variable", .{});
        return error.Unavailable;
    }

    var client = std.http.Client{ .allocator = allocator };
    defer std.http.Client.deinit(&client);

    const uri = try std.Uri.parse(try std.fmt.allocPrint(allocator, "https://api.openweathermap.org/data/2.5/weather?lat={}&lon={}&appid={s}", .{ 40.416775, -3.703790, key }));

    var header_buffer = [_]u8{0} ** 1024;

    var result = try std.http.Client.open(&client, .GET, uri, .{ .server_header_buffer = &header_buffer });
    defer result.deinit();

    try result.send();

    try result.wait();

    var reader = result.reader();

    const conents = try reader.readAllAlloc(allocator, 1_000);

    const parsed_contents = try std.json.parseFromSliceLeaky(WeatherResponse, allocator, conents, .{ .ignore_unknown_fields = true });

    std.debug.print("Country: {s}\n", .{parsed_contents.sys.country});
    std.debug.print("Sunrise: {d}\n", .{parsed_contents.sys.sunrise});
    std.debug.print("Sunset: {d}\n", .{parsed_contents.sys.sunset});

    // Get the current time and compare
    const current_time = @divFloor(std.time.milliTimestamp(), 1000);

    std.debug.print("Current time {?}\n", .{current_time});

    if (current_time > parsed_contents.sys.sunrise and current_time < parsed_contents.sys.sunset) {
        // TODO: detect bad wheater or bad visibility to turn on the light
        std.debug.print("It's day time\n", .{});
    } else {
        std.debug.print("It's night time\n", .{});
    }
}
