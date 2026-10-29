import { useLayoutEffect, useRef, useState } from "react";
import { monogram, tileColor } from "../lib/game-display";

interface CoverImageProps {
  /** asset URL (see `assetUrl`), or null for the monogram tile. */
  src: string | null;
  /** Title: alt text, monogram and tile colour. */
  title: string;
  /** Extra classes for the tile (size, aspect, radius). */
  className?: string;
  monogramClassName?: string;
  /** `contain` (covers: letterboxed, never cropped) or `cover` (hero snaps). */
  fit?: "contain" | "cover";
}

/** Art on the M1 tile colour. Covers use `object-fit: contain` (SNES box art is
 *  landscape → letterboxed, never cropped). Lazy, fades in; a missing or broken
 *  image shows the monogram. */
export function CoverImage({
  src,
  title,
  className = "",
  monogramClassName = "text-[40px]",
  fit = "contain",
}: CoverImageProps) {
  const [state, setState] = useState<"loading" | "loaded" | "failed">("loading");
  const imgRef = useRef<HTMLImageElement>(null);

  useLayoutEffect(() => {
    setState("loading");
    // Already decoded (memory cache on remount): no onLoad will fire.
    const img = imgRef.current;
    if (img?.complete && img.naturalWidth > 0) setState("loaded");
  }, [src]);

  const showImage = src !== null && state !== "failed";
  return (
    <div
      className={`relative flex items-center justify-center overflow-hidden ${className}`}
      style={{ backgroundColor: tileColor(title) }}
    >
      {state !== "loaded" && (
        <span aria-hidden="true" className={`font-display font-bold text-white/25 ${monogramClassName}`}>
          {monogram(title)}
        </span>
      )}
      {showImage && (
        <img
          ref={imgRef}
          src={src}
          alt={title}
          loading="lazy"
          decoding="async"
          draggable={false}
          onLoad={() => setState("loaded")}
          onError={() => setState("failed")}
          className={`absolute inset-0 size-full transition-opacity duration-200 ${fit === "cover" ? "object-cover" : "object-contain"} ${state === "loaded" ? "opacity-100" : "opacity-0"}`}
        />
      )}
    </div>
  );
}
