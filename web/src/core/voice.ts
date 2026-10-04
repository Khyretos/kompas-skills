// W4: the browser side of optional voice. Settings per device (a microphone and speakers
// belong to the device), a recorder for push-to-talk, and a reader that speaks a reply
// sentence by sentence. Audio is never stored: recordings go to the server once, spoken
// sentences live in object URLs that are revoked after playing.
// (Settings drafted by Qwen3.5 9B; the rest rewritten in review.)

export interface VoicePrefs {
  input: boolean; // the microphone button in the composer
  readAloud: boolean; // read replies aloud when they finish
  voice: string; // a Kokoro voice id
  lang: "" | "en" | "es" | "nl"; // "" lets Whisper detect the language
}

export const VOICE_DEFAULTS: VoicePrefs = { input: false, readAloud: false, voice: "af_heart", lang: "" };

const KEY = "kk-voice";

export function loadVoicePrefs(): VoicePrefs {
  try {
    const parsed = JSON.parse(localStorage.getItem(KEY) ?? "{}") as Partial<Record<keyof VoicePrefs, unknown>>;
    return {
      input: typeof parsed.input === "boolean" ? parsed.input : VOICE_DEFAULTS.input,
      readAloud: typeof parsed.readAloud === "boolean" ? parsed.readAloud : VOICE_DEFAULTS.readAloud,
      voice: typeof parsed.voice === "string" && parsed.voice ? parsed.voice : VOICE_DEFAULTS.voice,
      lang: parsed.lang === "en" || parsed.lang === "es" || parsed.lang === "nl" ? parsed.lang : "",
    };
  } catch {
    return { ...VOICE_DEFAULTS }; // storage blocked or not JSON
  }
}

export function saveVoicePrefs(p: VoicePrefs): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(p));
  } catch {
    /* storage blocked: the setting lasts until the page closes */
  }
}

const FENCE = new RegExp("`{3}[\\s\\S]*?(?:`{3}|$)", "g");

/** What to read aloud from a markdown reply: prose only (no code), in pieces of at most `max` characters. */
export function sentences(markdown: string, max = 300): string[] {
  const text = markdown
    .replace(FENCE, " ")
    .replace(/`([^`\n]*)`/g, "$1")
    .replace(/!\[[^\]]*\]\([^)]*\)/g, " ")
    .replace(/\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/^\s*(?:#{1,6}|>|[-*+]|\d+[.)])\s+/gm, "")
    .replace(/(\*\*|__)(.+?)\1/g, "$2")
    .replace(/(^|[\s(])[*_](\S(?:.*?\S)?)[*_](?=[\s).,!?;:]|$)/gm, "$1$2")
    .replace(/\s+/g, " ")
    .trim();
  if (!text) return [];
  const parts = text.split(/(?<=[.!?…])\s+/).map((p) => p.trim()).filter(Boolean);
  const out: string[] = [];
  let buf = "";
  const flush = () => { if (buf) out.push(buf); buf = ""; };
  for (let part of parts) {
    while (part.length > max) {
      flush();
      const cut = part.lastIndexOf(" ", max);
      const at = cut > 0 ? cut : max;
      out.push(part.slice(0, at).trim());
      part = part.slice(at).trim();
    }
    if (!part) continue;
    if (buf && buf.length + 1 + part.length > max) flush();
    buf = buf ? `${buf} ${part}` : part;
  }
  flush();
  return out;
}

/** Records the microphone until stop(). */
export class Recorder {
  private rec?: MediaRecorder;
  private chunks: Blob[] = [];
  private stream?: MediaStream;

  static supported(): boolean {
    return !!navigator.mediaDevices?.getUserMedia && typeof MediaRecorder !== "undefined";
  }

  get recording(): boolean {
    return this.rec?.state === "recording";
  }

  async start(): Promise<void> {
    this.stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    const mimeType = ["audio/webm;codecs=opus", "audio/ogg;codecs=opus", "audio/mp4"].find((t) => MediaRecorder.isTypeSupported(t));
    this.chunks = [];
    this.rec = new MediaRecorder(this.stream, mimeType ? { mimeType } : undefined);
    this.rec.addEventListener("dataavailable", (e) => { if (e.data.size > 0) this.chunks.push(e.data); });
    this.rec.start(250);
  }

  /** The recording; resolves after the recorder hands over its last piece. */
  stop(): Promise<Blob> {
    const rec = this.rec, stream = this.stream;
    this.rec = undefined;
    this.stream = undefined;
    if (!rec || rec.state === "inactive") {
      stream?.getTracks().forEach((t) => t.stop());
      return Promise.resolve(new Blob([], { type: "audio/webm" }));
    }
    return new Promise((resolve) => {
      rec.addEventListener("stop", () => {
        stream?.getTracks().forEach((t) => t.stop());
        resolve(new Blob(this.chunks, { type: rec.mimeType || "audio/webm" }));
        this.chunks = [];
      }, { once: true });
      rec.stop();
    });
  }
}

/** Speaks text sentence by sentence; the next sentence is fetched while one plays. */
export class Reader {
  speaking = false;
  private queue: string[] = [];
  private audio?: HTMLAudioElement;
  private next?: { text: string; blob: Promise<Blob> };
  private gen = 0;

  constructor(private speak: (text: string) => Promise<Blob>, private onChange: (speaking: boolean) => void) {}

  read(text: string): void {
    const parts = sentences(text);
    if (!parts.length) return;
    this.queue.push(...parts);
    if (!this.speaking) {
      this.speaking = true;
      this.onChange(true);
      void this.play();
    }
  }

  stop(): void {
    this.gen++;
    this.queue = [];
    this.next = undefined;
    this.audio?.pause();
    this.audio = undefined;
    if (this.speaking) {
      this.speaking = false;
      this.onChange(false);
    }
  }

  private async play(): Promise<void> {
    const g = this.gen;
    try {
      while (this.queue.length) {
        const text = this.queue.shift()!;
        const blob = this.next?.text === text ? this.next.blob : this.speak(text);
        const following = this.queue[0];
        this.next = following ? { text: following, blob: this.speak(following) } : undefined;
        this.next?.blob.catch(() => undefined); // an unused prefetch must not raise an unhandled rejection
        const audio = await blob;
        if (g !== this.gen) return;
        const url = URL.createObjectURL(audio);
        try {
          this.audio = new Audio(url);
          const done = new Promise<void>((resolve) => {
            this.audio!.addEventListener("ended", () => resolve(), { once: true });
            this.audio!.addEventListener("error", () => resolve(), { once: true });
          });
          await this.audio.play();
          await done;
        } finally {
          URL.revokeObjectURL(url);
        }
        if (g !== this.gen) return;
      }
      this.speaking = false;
      this.onChange(false);
    } catch {
      if (g === this.gen) this.stop();
    }
  }
}
