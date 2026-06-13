/**
 * Confirmation email — minimal dark template that doubles as the HTML
 * body (Resend accepts a `react` prop and renders to both HTML and
 * plaintext).
 *
 * Avoids leaderboard / position-bump language per the research
 * findings: this is a "welcome" email, not a "skip the line" email.
 */
import type { ReactElement } from "react";

interface Props {
  refCode: string;
  siteUrl: string;
}

export function ConfirmEmail({ refCode, siteUrl }: Props): ReactElement {
  const inviteUrl = `${siteUrl}/i/${refCode}`;
  return (
    <html lang="en">
      <body
        style={{
          margin: 0,
          padding: 0,
          backgroundColor: "#0B0B0B",
          color: "#CBC9BC",
          fontFamily:
            "'Inter', system-ui, -apple-system, BlinkMacSystemFont, sans-serif",
        }}
      >
        <table
          role="presentation"
          width="100%"
          cellPadding={0}
          cellSpacing={0}
          style={{ backgroundColor: "#0B0B0B" }}
        >
          <tbody>
            <tr>
              <td align="center" style={{ padding: "40px 20px" }}>
                <table
                  role="presentation"
                  width="560"
                  cellPadding={0}
                  cellSpacing={0}
                  style={{ maxWidth: "560px" }}
                >
                  <tbody>
                    {/* Wordmark */}
                    <tr>
                      <td style={{ paddingBottom: 32 }}>
                        <span
                          style={{
                            fontFamily: "'Merriweather', Georgia, serif",
                            fontWeight: 700,
                            fontSize: 22,
                            color: "#F5E6C8",
                            letterSpacing: "-0.02em",
                          }}
                        >
                          Mi
                          <span
                            style={{
                              color: "#D4A574",
                              margin: "0 1px",
                              display: "inline",
                            }}
                          >
                            ·
                          </span>
                          zan
                        </span>
                      </td>
                    </tr>
                    {/* Welcome */}
                    <tr>
                      <td>
                        <p
                          style={{
                            margin: "0 0 8px 0",
                            fontSize: 11,
                            letterSpacing: "0.08em",
                            textTransform: "uppercase",
                            color: "#D4A574",
                            fontWeight: 500,
                          }}
                        >
                          You&apos;re in
                        </p>
                        <h1
                          style={{
                            margin: "0 0 16px 0",
                            fontFamily: "'Merriweather', Georgia, serif",
                            fontSize: 32,
                            lineHeight: 1.15,
                            fontWeight: 700,
                            color: "#F5E6C8",
                          }}
                        >
                          You&apos;re on the list.
                        </h1>
                        <p
                          style={{
                            margin: "0 0 24px 0",
                            fontSize: 16,
                            lineHeight: 1.6,
                            color: "#CBC9BC",
                          }}
                        >
                          When Mizan launches, you&apos;ll get founding-member pricing locked in for life, priority access, and early influence on what we build next. We&apos;ll email you the moment it&apos;s ready.
                        </p>
                      </td>
                    </tr>
                    {/* Invite link */}
                    <tr>
                      <td style={{ paddingTop: 16 }}>
                        <div
                          style={{
                            border: "1px solid rgba(255,255,255,0.06)",
                            borderRadius: 12,
                            padding: 20,
                            backgroundColor: "#171717",
                          }}
                        >
                          <p
                            style={{
                              margin: "0 0 8px 0",
                              fontSize: 11,
                              letterSpacing: "0.08em",
                              textTransform: "uppercase",
                              color: "rgba(203, 201, 188, 0.55)",
                              fontWeight: 500,
                            }}
                          >
                            Your invite link
                          </p>
                          <p
                            style={{
                              margin: "0 0 12px 0",
                              fontFamily: "'IBM Plex Mono', monospace",
                              fontSize: 14,
                              wordBreak: "break-all",
                              color: "#F5E6C8",
                            }}
                          >
                            <a
                              href={inviteUrl}
                              style={{ color: "#F5E6C8", textDecoration: "none" }}
                            >
                              {inviteUrl}
                            </a>
                          </p>
                          <p
                            style={{
                              margin: 0,
                              fontSize: 12,
                              color: "rgba(203, 201, 188, 0.55)",
                              lineHeight: 1.5,
                            }}
                          >
                            Share if a friend would want this. No leaderboard, no position bumps — we don&apos;t play those games.
                          </p>
                        </div>
                      </td>
                    </tr>
                    {/* Signoff */}
                    <tr>
                      <td style={{ paddingTop: 32 }}>
                        <p
                          style={{
                            margin: 0,
                            fontSize: 16,
                            color: "#CBC9BC",
                            fontFamily: "'Merriweather', Georgia, serif",
                            fontStyle: "italic",
                          }}
                        >
                          — The Mizan team
                        </p>
                      </td>
                    </tr>
                    {/* Footer */}
                    <tr>
                      <td
                        style={{
                          paddingTop: 40,
                          borderTop: "1px solid rgba(255,255,255,0.06)",
                          marginTop: 32,
                        }}
                      >
                        <p
                          style={{
                            margin: "16px 0 0 0",
                            fontSize: 11,
                            color: "rgba(203, 201, 188, 0.4)",
                            letterSpacing: "0.05em",
                          }}
                        >
                          © 2026 Mizan · Made in Singapore
                        </p>
                      </td>
                    </tr>
                  </tbody>
                </table>
              </td>
            </tr>
          </tbody>
        </table>
      </body>
    </html>
  );
}
