/**
 * Build flags (spec 20 D18, E5 step 3): in the public build the Maps entry is hidden; in the
 * private build it is shown greyed out until path B exists.
 */
export function isPublicBuild(env: Record<string, unknown> = import.meta.env): boolean {
  return env.VITE_PUBLIC_BUILD === "1" || env.VITE_PUBLIC_BUILD === "true";
}
