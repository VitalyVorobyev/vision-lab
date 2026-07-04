export type PointF32 = {
  x: number;
  y: number;
};

export type RectF32 = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export function rectFromPoints(a: PointF32, b: PointF32): RectF32 {
  return {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    width: Math.abs(a.x - b.x),
    height: Math.abs(a.y - b.y),
  };
}

export function isUsableRect(rect: RectF32 | null): rect is RectF32 {
  return rect !== null && rect.width >= 2 && rect.height >= 2;
}
