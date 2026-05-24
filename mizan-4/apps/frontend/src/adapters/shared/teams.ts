// Teams adapter (M5.2).
//
// Reads the cloud's /v1/me/teams and /v1/teams/:id/members endpoints
// through the existing ConnectApiClient (proxied via the
// `get_my_teams` / `get_team_members` Tauri commands, added below).
//
// Pro+ on the cloud side: 401 for signed-out / missing-subscription
// callers. The frontend treats a 401 the same as "no teams" so a Free
// user who somehow lands on /advisor just sees an empty state rather
// than an error banner.

import { invoke } from "./platform";

/** Mirrors cloud's `TeamSummary`. */
export interface TeamSummary {
  id: string;
  name: string;
  ownerUserId: string;
  brandingLogoUrl: string | null;
  brandingColor: string | null;
  /** Caller's role on this team. Always present for /me/teams responses. */
  myRole: "owner" | "advisor" | "viewer" | null;
  createdAt: string | null;
}

/** Mirrors cloud's `TeamMemberDto`. */
export interface TeamMemberDto {
  userId: string;
  role: string;
  joinedAt: string | null;
  displayName: string | null;
}

export interface MyTeamsResponse {
  teams: TeamSummary[];
}

export interface TeamMembersResponse {
  teamId: string;
  members: TeamMemberDto[];
}

export async function listMyTeams(): Promise<MyTeamsResponse> {
  return invoke<MyTeamsResponse>("list_my_teams");
}

export async function listTeamMembers(teamId: string): Promise<TeamMembersResponse> {
  return invoke<TeamMembersResponse>("list_team_members", { teamId });
}
