const std = @import("std");
const Allocator = std.mem.Allocator;
const assert = std.debug.assert;

const error_log_print = "ZIG ERROR: {s}\n";

const PossibleReturnValues = enum(u2) {
    ok,
    not_okey,
    crash,
};
const TrueString = "True";
const FalseString = "False";
const CrashString = "Crash";

const ReturnValue = struct {
    diagnosticPositive: PossibleReturnValues,
    pub fn init(value: PossibleReturnValues) ReturnValue {
        return ReturnValue{ .diagnosticPositive = value };
    }
    pub fn calculate_return_value_string(self: ReturnValue) []const u8 {
        switch (self.diagnosticPositive) {
            PossibleReturnValues.ok => return TrueString[0..],
            PossibleReturnValues.not_okey => return FalseString[0..],
            PossibleReturnValues.crash => return CrashString[0..],
        }
    }
    pub fn allocate_status_result(allocator: Allocator, result: []const u8) [*:0]const u8 {
        const result_c_string: [*:0]u8 = allocator.allocSentinel(u8, result.len + 1, 0) catch {
            return FalseString;
        };
        errdefer allocator.free(result_c_string);
        @memcpy(result_c_string[0..result.len], result);
        result_c_string[result.len] = 0;
        return result_c_string;
    }
};

const Pool = std.Thread.Pool;
const WaitGroup = std.Thread.WaitGroup;

const ThreadedRequestProcessor = struct {
    pub fn send_request(
        client: *std.http.Client,
        allocator: Allocator,
        uri: std.Uri,
        mutex: *std.Thread.Mutex,
        array_list: *std.ArrayList(u2),
    ) void {
        errdefer {
            mutex.lock();
            array_list.append(@intFromEnum(PossibleReturnValues.crash)) catch unreachable;
            mutex.unlock();
        }

        const header_buffer = allocator.alloc(u8, 1_000) catch unreachable;
        defer allocator.free(header_buffer);

        mutex.lock();
        defer mutex.unlock();
        var request = std.http.Client.open(
            client,
            .GET,
            uri,
            .{
                .server_header_buffer = header_buffer,
            },
        ) catch {
            array_list.append(@intFromEnum(PossibleReturnValues.crash)) catch unreachable;
            return;
        };
        defer request.deinit();

        request.send() catch {
            array_list.append(@intFromEnum(PossibleReturnValues.crash)) catch unreachable;
            return;
        };

        {
            mutex.unlock();
            defer mutex.lock();
            request.wait() catch {
                mutex.lock();
                defer mutex.unlock();
                array_list.append(@intFromEnum(PossibleReturnValues.crash)) catch unreachable;
                return;
            };
        }

        switch (request.response.status) {
            .ok => array_list.append(@intFromEnum(PossibleReturnValues.ok)) catch unreachable,
            .moved_permanently => array_list.append(@intFromEnum(PossibleReturnValues.ok)) catch unreachable,
            else => array_list.append(@intFromEnum(PossibleReturnValues.not_okey)) catch unreachable,
        }
    }
};

pub fn main() !void {
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena.deinit();
    const allocator = arena.allocator();
    const result = try bitseaarch(allocator);

    var return_value = ReturnValue.init(result);

    std.debug.print("Result: {s}\n", .{return_value.calculate_return_value_string()});
}

export fn free_string(s: [*:0]const u8) void {
    const c_allocator = std.heap.c_allocator;
    const lenght = std.mem.len(s);

    const value = s[0..lenght];

    c_allocator.free(value);
}

export fn start() [*]const u8 {
    var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
    defer arena.deinit();
    const allocator = arena.allocator();

    // Dont free this memory as we are returning it to the caller
    const c_allocator = std.heap.c_allocator;

    const result = bitseaarch(allocator) catch |error_out| {
        switch (error_out) {
            error.SystemResources => std.debug.print(error_log_print, .{"The method returned a SystemResources error. Most likely the system ran out of memory."}),
            error.Unexpected => std.debug.print(error_log_print, .{"The method returned an Unexpected error. No more info, sorry."}),
            error.OutOfMemory => std.debug.print(error_log_print, .{"The method returned an OutOfMemory error. Most likely the system ran out of memory."}),
            error.ThreadQuotaExceeded => std.debug.print(error_log_print, .{"The method returned a ThreadQuotaExceeded error. Most likely the system ran out of memory."}),
            error.LockedMemoryLimitExceeded => std.debug.print(error_log_print, .{"The method returned a LockedMemoryLimitExceeded error. Most likely the system ran out of memory."}),
        }
        return ReturnValue.allocate_status_result(
            c_allocator,
            ReturnValue.init(PossibleReturnValues.crash).calculate_return_value_string(),
        );
    };

    const result_value = ReturnValue{ .diagnosticPositive = result };

    const allocated_return_value = ReturnValue.allocate_status_result(
        c_allocator,
        result_value.calculate_return_value_string(),
    );
    return allocated_return_value;
}

fn bitseaarch(allocator: Allocator) !PossibleReturnValues {
    var client = std.http.Client{ .allocator = allocator };
    defer std.http.Client.deinit(&client);

    const home_page_uri = try comptime std.Uri.parse("https://bitsearch.to/");
    const about_page_uri = try comptime std.Uri.parse("https://bitsearch.to/about/");
    const library_page_uri = try comptime std.Uri.parse("https://bitsearch.to/library/");
    const library_page_two_uri = try comptime std.Uri.parse("https://bitsearch.to/library?year=2020-&page=2");

    const uri_list = [_]std.Uri{ home_page_uri, about_page_uri, library_page_uri, library_page_two_uri };

    var single_threaded_arena = std.heap.ArenaAllocator.init(std.heap.c_allocator);
    defer single_threaded_arena.deinit();

    var thread_safe_arena: std.heap.ThreadSafeAllocator = .{
        .child_allocator = single_threaded_arena.allocator(),
    };

    const arena = thread_safe_arena.allocator();

    var thread_pool: Pool = undefined;
    try thread_pool.init(Pool.Options{
        .allocator = arena,
    });
    defer thread_pool.deinit();

    var wait_group = WaitGroup{};
    wait_group.reset();

    var mutex = std.Thread.Mutex{};

    var result_array_list = std.ArrayList(u2).init(arena);
    defer result_array_list.deinit();

    for (uri_list) |uri| {
        thread_pool.spawnWg(
            &wait_group,
            ThreadedRequestProcessor.send_request,
            .{
                &client,
                arena,
                uri,
                &mutex,
                &result_array_list,
            },
        );
    }
    thread_pool.waitAndWork(&wait_group);

    assert(result_array_list.items.len == uri_list.len);

    for (result_array_list.items) |result| {
        const converted_value: PossibleReturnValues = @enumFromInt(result);
        switch (converted_value) {
            .ok => continue,
            .not_okey => return PossibleReturnValues.not_okey,
            .crash => {
                std.debug.print(error_log_print, .{"The thread added a crash result to the resulting arraylist."});
                return PossibleReturnValues.crash;
            },
        }
    }

    return PossibleReturnValues.ok;
}
