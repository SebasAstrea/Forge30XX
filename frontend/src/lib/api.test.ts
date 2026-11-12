// Tests del parser de eventos SSE del contrato UI↔backend (ADR-0015).
import { describe, expect, it } from "vitest";
import { parseBlock } from "./api";

describe("parseBlock", () => {
  it("parses a token event", () => {
    const ev = parseBlock('event: token\ndata: {"text":"fn "}');
    expect(ev).toEqual({ kind: "token", text: "fn " });
  });

  it("parses a destructive tool_request", () => {
    const ev = parseBlock(
      'event: tool_request\ndata: {"id":"t1","name":"delete_file","arguments":{"path":"a"},"destructive":true}',
    );
    expect(ev).toEqual({
      kind: "tool_request",
      request: { id: "t1", name: "delete_file", arguments: { path: "a" }, destructive: true },
    });
  });

  it("parses tool_result, error and done", () => {
    expect(parseBlock('event: tool_result\ndata: {"id":"t1","status":"rejected"}')).toEqual({
      kind: "tool_result",
      id: "t1",
      status: "rejected",
    });
    expect(parseBlock('event: error\ndata: {"message":"boom"}')).toEqual({
      kind: "error",
      message: "boom",
    });
    expect(parseBlock("event: done\ndata: {}")).toEqual({ kind: "done" });
  });

  it("returns null for a block without an event name", () => {
    expect(parseBlock('data: {"text":"x"}')).toBeNull();
  });
});
