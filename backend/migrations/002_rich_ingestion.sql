-- 002: rich ingestion — review threads, comments, unique constraints, failure surfacing
--
-- (1) Unique constraints make the existing `ON CONFLICT DO NOTHING` inserts actually
--     de-duplicate, so re-running analysis is safe.
-- (2) New tables capture PR review threads + PR/issue comments — the real debate
--     material the analysis prompt was designed around but never received.
-- (3) repos.error_message + repos.stage give the frontend real failure reasons and
--     real progress instead of fake streams.

-- ---- de-duplicate any rows that slipped in before the constraints existed ----
DELETE FROM commits a USING commits b
  WHERE a.repo_id = b.repo_id AND a.sha = b.sha AND a.id > b.id;

DELETE FROM pull_requests a USING pull_requests b
  WHERE a.repo_id = b.repo_id AND a.number = b.number AND a.id > b.id;

DELETE FROM issues a USING issues b
  WHERE a.repo_id = b.repo_id AND a.number = b.number AND a.id > b.id;

-- ---- unique constraints ----
ALTER TABLE commits
  ADD CONSTRAINT uq_commits_repo_sha UNIQUE (repo_id, sha);

ALTER TABLE pull_requests
  ADD CONSTRAINT uq_pull_requests_repo_number UNIQUE (repo_id, number);

ALTER TABLE issues
  ADD CONSTRAINT uq_issues_repo_number UNIQUE (repo_id, number);

-- ---- failure + progress surfacing ----
ALTER TABLE repos ADD COLUMN IF NOT EXISTS error_message TEXT;
ALTER TABLE repos ADD COLUMN IF NOT EXISTS stage TEXT;

-- ---- PR review threads ----
CREATE TABLE IF NOT EXISTS pull_request_reviews (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    repo_id UUID REFERENCES repos(id) ON DELETE CASCADE,
    pr_number INT,
    github_id BIGINT,
    user TEXT,
    state TEXT,
    body TEXT,
    submitted_at TIMESTAMP,
    CONSTRAINT uq_pr_reviews UNIQUE (repo_id, github_id)
);

CREATE TABLE IF NOT EXISTS pull_request_comments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    repo_id UUID REFERENCES repos(id) ON DELETE CASCADE,
    pr_number INT,
    github_id BIGINT,
    user TEXT,
    body TEXT,
    path TEXT,
    created_at TIMESTAMP,
    CONSTRAINT uq_pr_comments UNIQUE (repo_id, github_id)
);

-- ---- issue comments ----
CREATE TABLE IF NOT EXISTS issue_comments (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    repo_id UUID REFERENCES repos(id) ON DELETE CASCADE,
    issue_number INT,
    github_id BIGINT,
    user TEXT,
    body TEXT,
    created_at TIMESTAMP,
    CONSTRAINT uq_issue_comments UNIQUE (repo_id, github_id)
);

CREATE INDEX idx_pr_reviews_repo ON pull_request_reviews(repo_id, pr_number);
CREATE INDEX idx_pr_comments_repo ON pull_request_comments(repo_id, pr_number);
CREATE INDEX idx_issue_comments_repo ON issue_comments(repo_id, issue_number);
