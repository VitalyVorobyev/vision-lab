export type OverlayKey = "roi" | "points" | "bbox" | "summary";

export type OverlayVisibility = Record<OverlayKey, boolean>;

export const defaultOverlayVisibility: OverlayVisibility = {
  roi: true,
  points: true,
  bbox: true,
  summary: true,
};
