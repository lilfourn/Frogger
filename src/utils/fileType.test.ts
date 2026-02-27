import { describe, it, expect } from "vitest";
import { detectType, isImageFile } from "./fileType";

describe("detectType", () => {
  it("returns image for image extensions", () => {
    expect(detectType("photo.png")).toBe("image");
    expect(detectType("photo.jpg")).toBe("image");
    expect(detectType("photo.jpeg")).toBe("image");
    expect(detectType("photo.gif")).toBe("image");
    expect(detectType("photo.webp")).toBe("image");
    expect(detectType("photo.svg")).toBe("image");
    expect(detectType("photo.bmp")).toBe("image");
    expect(detectType("photo.ico")).toBe("image");
  });

  it("returns video for video extensions", () => {
    expect(detectType("clip.mp4")).toBe("video");
    expect(detectType("clip.webm")).toBe("video");
    expect(detectType("clip.mov")).toBe("video");
  });

  it("returns audio for audio extensions", () => {
    expect(detectType("song.mp3")).toBe("audio");
    expect(detectType("song.wav")).toBe("audio");
    expect(detectType("song.flac")).toBe("audio");
  });

  it("returns code for code extensions", () => {
    expect(detectType("app.ts")).toBe("code");
    expect(detectType("app.tsx")).toBe("code");
    expect(detectType("app.py")).toBe("code");
    expect(detectType("app.rs")).toBe("code");
  });

  it("returns markdown for md files", () => {
    expect(detectType("readme.md")).toBe("markdown");
    expect(detectType("readme.markdown")).toBe("markdown");
  });

  it("returns pdf for pdf files", () => {
    expect(detectType("doc.pdf")).toBe("pdf");
  });

  it("returns unknown for null", () => {
    expect(detectType(null)).toBe("unknown");
  });

  it("returns unknown for unrecognized extensions", () => {
    expect(detectType("file.xyz")).toBe("unknown");
    expect(detectType("noext")).toBe("unknown");
  });

  it("handles full paths", () => {
    expect(detectType("/Users/me/photos/cat.png")).toBe("image");
  });

  it("is case-insensitive", () => {
    expect(detectType("photo.PNG")).toBe("image");
    expect(detectType("photo.JPG")).toBe("image");
  });
});

describe("isImageFile", () => {
  it("returns true for image files", () => {
    expect(isImageFile("photo.png")).toBe(true);
    expect(isImageFile("photo.jpg")).toBe(true);
    expect(isImageFile("/path/to/photo.webp")).toBe(true);
  });

  it("returns false for non-image files", () => {
    expect(isImageFile("doc.pdf")).toBe(false);
    expect(isImageFile("app.ts")).toBe(false);
    expect(isImageFile("noext")).toBe(false);
  });
});
