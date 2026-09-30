/** Shapes `gh --json` returns, trimmed to what the GitHub view reads. */
export interface Actor { login: string; name?: string; is_bot?: boolean }
export interface Label { name: string; color: string; description?: string }
export interface Comment { id?: string; author: Actor | null; body: string; createdAt: string; url?: string }
export interface Review { id?: string; author: Actor | null; body: string; state: ReviewState; submittedAt: string }
export type ReviewState = 'APPROVED' | 'CHANGES_REQUESTED' | 'COMMENTED' | 'DISMISSED' | 'PENDING'
export interface Check {
  __typename: 'CheckRun' | 'StatusContext'
  name?: string; context?: string; workflowName?: string
  status?: string; conclusion?: string; state?: string
  detailsUrl?: string; targetUrl?: string; startedAt?: string; completedAt?: string
}
export interface Commit { oid: string; messageHeadline: string; messageBody?: string; authoredDate: string; authors: Actor[] }

export interface PrSummary {
  number: number; title: string; state: 'OPEN' | 'CLOSED' | 'MERGED'; isDraft: boolean; url: string
  author: Actor | null; headRefName: string; baseRefName: string
  reviewDecision: '' | 'APPROVED' | 'CHANGES_REQUESTED' | 'REVIEW_REQUIRED' | null
  statusCheckRollup: Check[] | null; additions: number; deletions: number
  updatedAt: string; createdAt: string; labels: Label[]; comments: Comment[]; assignees: Actor[]
}
export interface Pr extends PrSummary {
  body: string; headRefOid: string; isCrossRepository: boolean; headRepositoryOwner: Actor | null
  mergeable: 'MERGEABLE' | 'CONFLICTING' | 'UNKNOWN'; mergeStateStatus: string
  changedFiles: number; commits: Commit[]; reviews: Review[]; latestReviews: Review[]
  reviewRequests: { login?: string; name?: string; slug?: string }[]
  closingIssuesReferences: { number: number; url: string }[]
  mergedAt: string | null; mergedBy: Actor | null; milestone: { title: string } | null
  autoMergeRequest: unknown | null
}
export interface IssueSummary {
  number: number; title: string; state: 'OPEN' | 'CLOSED'; stateReason: string | null; url: string
  author: Actor | null; labels: Label[]; assignees: Actor[]; comments: Comment[]; updatedAt: string; createdAt: string
}
export interface Issue extends IssueSummary {
  body: string; closedAt: string | null; milestone: { title: string } | null
  closedByPullRequestsReferences: { number: number; url: string }[]
}

export interface Repo { id: string; owner: string; name: string; nameWithOwner: string; url: string; defaultBranch: string; viewerPermission: string; isFork: boolean }
export interface GhStatus { installed: boolean; version: string | null; signedIn: boolean; account: string | null; scopes: string[]; repo: Repo | null; problem: string | null }
export interface RepoMeta { labels: Label[]; assignees: string[] }

export interface ProjectSummary { id: string; number: number; title: string; url: string; owner: string; description: string; items: number; linked: boolean }
export interface Choice { id: string; name: string; color: string; description: string }
export interface SelectField { id: string; name: string; options: Choice[] }
export interface Card {
  id: string; kind: 'issue' | 'pr' | 'draft' | 'redacted'; number: number | null; title: string
  state: string | null; url: string | null; repo: string | null; draftPr: boolean; body: string | null
  assignees: string[]; labels: Label[]; values: Record<string, string>
}
export interface Board { id: string; title: string; url: string; fields: SelectField[]; cards: Card[]; truncated: boolean }
export interface PrDraft { branch: string; base: string; title: string; body: string; commits: number; pushed: boolean; remote: string }

export type Section = 'prs' | 'issues' | 'boards'
export type Filter = 'open' | 'closed' | 'merged' | 'all'
export type Kind = 'pr' | 'issue'
export interface MenuItem { label: string; hint?: string; icon?: unknown; danger?: boolean; disabled?: boolean; run: () => unknown }
