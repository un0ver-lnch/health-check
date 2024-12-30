const std = @import("std");

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

    std.debug.print("Key: {s}\n", .{key});

    var client = std.http.Client{ .allocator = allocator };
    defer std.http.Client.deinit(&client);

    const uri = try std.Uri.parse(try std.fmt.allocPrint(allocator, "https://api.openweathermap.org/data/3.0/onecall?lat={}&lon={}&appid={s}", .{ 40.416775, -3.703790, key }));

    var header_buffer = [_]u8{0} ** 1024;

    var result = try std.http.Client.open(&client, .GET, uri, .{ .server_header_buffer = &header_buffer });
    defer result.deinit();

    try result.wait();
    var body_buffer = [_]u8{0} ** 1024;

    while (try result.read(&body_buffer) == 1024) {
        std.debug.print("{s}", .{body_buffer});
    }
    std.debug.print("{s}", .{body_buffer});
}
