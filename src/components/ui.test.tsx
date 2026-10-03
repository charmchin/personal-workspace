// @vitest-environment jsdom
import { cleanup, fireEvent, render, renderHook, screen, waitFor, act } from "@testing-library/react";
import { afterEach, beforeAll, expect, it, vi } from "vitest";
import { useState } from "react";
import { Button, Field, Input, Modal } from "./ui";
import { useMutation } from "../lib/useMutation";
import { WorkbenchError } from "../lib/api";
import { useSearchTarget } from "../lib/useSearchTarget";
import type { SearchResult } from "../types";

beforeAll(() => {
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
});
afterEach(cleanup);

it("保存期间禁止重复提交及关闭，失败后表单可继续编辑且错误在弹窗内", async () => {
  let release!: () => void;
  const operation = vi.fn(() => new Promise<void>((resolve) => { release = resolve; }));
  const close = vi.fn();
  function Harness() {
    const [error, setError] = useState<WorkbenchError | null>(null);
    const { busy, mutate } = useMutation(setError);
    return <Modal open onClose={close} title="编辑记录" busy={busy} error={error} footer={<Button onClick={() => void mutate(async () => { await operation(); throw new WorkbenchError({ code: "TEST", message: "标题不能为空", recovery: "填写标题后重试" }); })}>保存</Button>}><Field label="标题"><Input /></Field></Modal>;
  }
  render(<Harness />);
  const button = screen.getByRole("button", { name: "保存" });
  fireEvent.click(button); fireEvent.click(button);
  expect(operation).toHaveBeenCalledTimes(1);
  expect(screen.getByLabelText("标题").closest("fieldset")?.disabled).toBe(true);
  fireEvent(screen.getByRole("dialog"), new Event("cancel", { bubbles: true, cancelable: true }));
  expect(close).not.toHaveBeenCalled();
  await act(async () => { release(); });
  await waitFor(() => expect(screen.getByRole("alert").closest("dialog")).not.toBeNull());
  expect(screen.getByRole("alert").textContent).toContain("标题不能为空");
  expect(button.hasAttribute("disabled")).toBe(false);
  expect(screen.getByRole("alert").parentElement).toBe(document.activeElement);
});

it("同一个搜索请求只打开一次，重新选择同一记录仍可再次打开", () => {
  const record = { id: "one" }; const records = [record]; const open = vi.fn();
  const target: SearchResult = { id: "one", kind: "habit", title: "阅读", subtitle: "daily" };
  const { rerender } = renderHook(({ selection }) => useSearchTarget(selection, "habit", records, open), { initialProps: { selection: target } });
  expect(open).toHaveBeenCalledWith(record);
  rerender({ selection: target }); expect(open).toHaveBeenCalledTimes(1);
  rerender({ selection: { ...target } }); expect(open).toHaveBeenCalledTimes(2);
});
