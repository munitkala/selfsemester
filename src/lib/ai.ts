// Small helpers for showing who does which job.
import type { AiView, Roles, Target } from "#lib/api.ts";

export type Job = keyof Roles;

export const isClaude = (t: Target) => !t.service || t.service === "claude_code";

/** How many of the four jobs Claude Code does. */
export function jobsOnClaude(roles: Roles): number {
  return [roles.plan, roles.write, roles.check, roles.research].filter(isClaude).length;
}

/** "Claude Code", or the service's name and the model it uses for that job. */
export function whoDoes(ai: AiView, job: Job): string {
  const t = ai.roles[job];
  if (isClaude(t)) return "Claude Code";
  const s = ai.services.find((x) => x.id === t.service);
  if (!s) return "Claude Code";
  const model = t.model || s.model;
  return model ? `${s.name || "your service"} (${model})` : s.name || "your service";
}
