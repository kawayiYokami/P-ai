declare module "culori" {
  export type OklchColor = {
    mode: "oklch";
    l: number;
    c: number;
    h: number;
    alpha?: number;
  };

  export type RgbColor = {
    mode: "rgb";
    r: number;
    g: number;
    b: number;
    alpha?: number;
  };

  export function converter(mode: "rgb"): (color: unknown) => RgbColor | null;
  export function converter(mode: "oklch"): (color: unknown) => OklchColor | null;
}
