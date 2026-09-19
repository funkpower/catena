/**
 * Decide playability and build the ffmpeg call the way the media server did,
 * for comparison.
 *
 * The functions below are the originals, copied out of the transcode service
 * verbatim and with their type annotations dropped — the module they live in
 * opens a database connection when it loads, which an oracle must not do. The
 * surrounding job bookkeeping (Prisma writes, progress parsing, the temporary
 * file it renames from) is not copied, because none of it decides anything.
 *
 *     node oracle/compatible.mjs < cases.ndjson
 *
 * One case per line: {path, video, audio, encoder, output}. Writes one JSON
 * verdict per line.
 */

import { extname } from "node:path";
import { createInterface } from "node:readline";

const APPLE_NATIVE_VIDEO = /^(avc1|h264|hvc1|hev1|hevc)/i;
const APPLE_NATIVE_AUDIO = /^(aac|mp4a)/i;
const MP4_CONTAINERS = new Set([".mp4", ".m4v", ".mov"]);

function isAppleCompatible(videoCodec, audioCodec, videoPath) {
  if (!videoCodec || !audioCodec || !videoPath) return false;
  return (
    APPLE_NATIVE_VIDEO.test(videoCodec) &&
    APPLE_NATIVE_AUDIO.test(audioCodec) &&
    MP4_CONTAINERS.has(extname(videoPath).toLowerCase())
  );
}

/**
 * The encoder table. The original reads COMPATIBLE_VIDEO_ENCODER here; the
 * name is passed in instead, so one run can cover every branch.
 */
function videoEncodePlan(name, device) {
  switch ((name || "h264_nvenc").toLowerCase()) {
    case "h264_videotoolbox":
      return {
        preInput: [],
        video: ["-c:v", "h264_videotoolbox", "-q:v", "60", "-profile:v", "high", "-pix_fmt", "yuv420p"],
      };
    case "h264_vaapi":
      return {
        preInput: ["-init_hw_device", `vaapi=va:${device || "/dev/dri/renderD128"}`, "-filter_hw_device", "va"],
        video: ["-vf", "format=nv12,hwupload", "-c:v", "h264_vaapi", "-qp", "23"],
      };
    case "libx264":
      return {
        preInput: [],
        video: ["-c:v", "libx264", "-preset", "veryfast", "-crf", "23", "-pix_fmt", "yuv420p"],
      };
    default:
      return {
        preInput: [],
        video: ["-c:v", "h264_nvenc", "-preset", "p5", "-rc", "vbr", "-cq", "23", "-b:v", "0"],
      };
  }
}

function verdict(c) {
  const vcodec = c.video || "";
  const acodec = c.audio || "";

  if (isAppleCompatible(vcodec, acodec, c.path)) {
    return { path: c.path, playable: true, encoder: null, copies_audio: true, argv: [] };
  }

  const copyVideo = APPLE_NATIVE_VIDEO.test(vcodec);
  const copyAudio = APPLE_NATIVE_AUDIO.test(acodec);
  const base = c.path.slice(0, c.path.length - extname(c.path).length);
  const out = c.output || `${base}.h264.mp4`;

  const plan = copyVideo
    ? { preInput: [], video: ["-c:v", "copy"] }
    : videoEncodePlan(c.encoder, c.vaapi_device);
  const encoder = copyVideo ? "copy" : (c.encoder || "h264_nvenc");

  const argv = [
    "-nostdin",
    "-y",
    ...plan.preInput,
    "-i",
    c.path,
    ...plan.video,
    ...(copyAudio ? ["-c:a", "copy"] : ["-c:a", "aac", "-b:a", "160k", "-ac", "2"]),
    "-movflags",
    "+faststart",
    out,
  ];
  return { path: c.path, playable: false, encoder, copies_audio: copyAudio, argv };
}

const lines = createInterface({ input: process.stdin, crlfDelay: Infinity });
for await (const line of lines) {
  if (!line.trim()) continue;
  process.stdout.write(JSON.stringify(verdict(JSON.parse(line))) + "\n");
}
