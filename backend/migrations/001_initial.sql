CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE repos (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    github_url TEXT NOT NULL,
    owner TEXT,
    name TEXT,
    analyzed_at TIMESTAMP,
    status TEXT DEFAULT 'pending'
);

CREATE TABLE commits (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    repo_id UUID REFERENCES repos(id) ON DELETE CASCADE,
    sha TEXT,
    message TEXT,
    author TEXT,
    timestamp TIMESTAMP,
    files_changed JSONB
);

CREATE TABLE pull_requests (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    repo_id UUID REFERENCES repos(id) ON DELETE CASCADE,
    number INT,
    title TEXT,
    body TEXT,
    state TEXT,
    author TEXT,
    created_at TIMESTAMP,
    merged_at TIMESTAMP
);

CREATE TABLE issues (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    repo_id UUID REFERENCES repos(id) ON DELETE CASCADE,
    number INT,
    title TEXT,
    body TEXT,
    state TEXT,
    author TEXT,
    created_at TIMESTAMP,
    comments JSONB
);

CREATE TABLE intent_nodes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    repo_id UUID REFERENCES repos(id) ON DELETE CASCADE,
    node_type TEXT,
    title TEXT,
    summary TEXT,
    reasoning TEXT,
    contributors JSONB,
    source_refs JSONB,
    timestamp TIMESTAMP,
    confidence FLOAT
);

CREATE INDEX idx_commits_repo_id ON commits(repo_id);
CREATE INDEX idx_pull_requests_repo_id ON pull_requests(repo_id);
CREATE INDEX idx_issues_repo_id ON issues(repo_id);
CREATE INDEX idx_intent_nodes_repo_id ON intent_nodes(repo_id);
