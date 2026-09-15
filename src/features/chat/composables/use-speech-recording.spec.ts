import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useSpeechRecording } from "./use-speech-recording";

type Deferred<T> = {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (error: unknown) => void;
};

function createDeferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

class FakeMediaRecorder {
  static instances: FakeMediaRecorder[] = [];
  state = "inactive";
  mimeType = "audio/webm";
  stopCalls = 0;
  ondataavailable: ((event: { data: unknown }) => void) | null = null;
  onerror: (() => void) | null = null;
  onstop: (() => void) | null = null;

  constructor(public stream: unknown) {
    FakeMediaRecorder.instances.push(this);
  }

  start() {
    this.state = "recording";
  }

  stop() {
    this.stopCalls += 1;
    this.state = "inactive";
    const onstop = this.onstop;
    queueMicrotask(() => {
      onstop?.();
    });
  }
}

function fakeStream() {
  return { getTracks: () => [{ stop: () => {} }] };
}

function buildOptions() {
  return {
    t: (key: string) => key,
    canStart: () => true,
    getLanguage: () => "zh-CN",
    getMinRecordSeconds: () => 1,
    getMaxRecordSeconds: () => 60,
    shouldUseRemoteStt: () => true,
    transcribeRemoteStt: async () => "识别文本",
    appendRecognizedText: () => {},
    setStatus: () => {},
  };
}

describe("useSpeechRecording 启动期间的停止请求", () => {
  let getUserMediaDeferred: Deferred<unknown>;

  beforeEach(() => {
    FakeMediaRecorder.instances = [];
    getUserMediaDeferred = createDeferred<unknown>();
    Object.defineProperty(globalThis, "navigator", {
      value: { mediaDevices: { getUserMedia: () => getUserMediaDeferred.promise } },
      configurable: true,
      writable: true,
    });
    Object.defineProperty(globalThis, "MediaRecorder", {
      value: FakeMediaRecorder,
      configurable: true,
      writable: true,
    });
  });

  afterEach(() => {
    delete (globalThis as { MediaRecorder?: unknown }).MediaRecorder;
  });

  it("启动未落地时的停止请求会被挂起，并在录音开始后立刻执行", async () => {
    const recording = useSpeechRecording(buildOptions());

    const startPromise = recording.startRecording();
    // 此刻 getUserMedia 还没返回，录音尚未开始
    await Promise.resolve();
    expect(recording.recording.value).toBe(false);

    await recording.stopRecording(true);

    getUserMediaDeferred.resolve(fakeStream());
    await startPromise;
    // 等 onstop 的微任务落地
    await Promise.resolve();
    await Promise.resolve();

    expect(FakeMediaRecorder.instances).toHaveLength(1);
    expect(FakeMediaRecorder.instances[0].stopCalls).toBe(1);
    expect(recording.recording.value).toBe(false);

    recording.cleanup();
  });

  it("启动已落地时的停止请求直接生效", async () => {
    const recording = useSpeechRecording(buildOptions());

    const startPromise = recording.startRecording();
    getUserMediaDeferred.resolve(fakeStream());
    await startPromise;
    expect(recording.recording.value).toBe(true);

    await recording.stopRecording(false);
    await Promise.resolve();
    await Promise.resolve();

    expect(FakeMediaRecorder.instances[0].stopCalls).toBe(1);
    expect(recording.recording.value).toBe(false);

    recording.cleanup();
  });

  it("空闲状态收到的停止不会残留成后续录音的停止意图", async () => {
    const recording = useSpeechRecording(buildOptions());

    // 未启动即收到停止：必须直接忽略，不能记成待停意图
    await recording.stopRecording(true);

    const startPromise = recording.startRecording();
    getUserMediaDeferred.resolve(fakeStream());
    await startPromise;

    expect(recording.recording.value).toBe(true);
    expect(FakeMediaRecorder.instances[0].stopCalls).toBe(0);

    recording.cleanup();
  });
});
