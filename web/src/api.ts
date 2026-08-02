export interface VaultInfo {
  name: string;
  path: string;
}

/**
 * A note as the API describes it. `key` — the vault-relative path — is the
 * address used by every other call. `title` is for display only and is *not*
 * unique: one vault can hold many files called `README.md`.
 */
export interface NoteInfo {
  key: string;
  title: string;
  /** Pulled in from a `scope.include` directory: read-only. */
  reference: boolean;
}

export interface NoteContent extends NoteInfo {
  content: string;
}

export interface SearchHit {
  vault: string;
  title: string;
  path: string;
  snippet: string;
}

/** A `[[target]]` as written, with the note path(s) it resolves to. */
export interface ForwardLink {
  target: string;
  keys: string[];
}

export interface NoteRef {
  key: string;
  title: string;
}

export interface NoteLinks {
  forward: ForwardLink[];
  backlinks: NoteRef[];
  cross_vault_backlinks: string[];
}

export interface GraphNode {
  /** Note path, prefixed with the vault name in all-vaults mode. */
  id: string;
  label: string;
  /** A wikilink target with no note behind it. */
  missing: boolean;
  reference: boolean;
}

export interface GraphData {
  nodes: GraphNode[];
  edges: { from: string; to: string }[];
}

export interface DoctorReport {
  vault: string;
  /// What the vault says about itself — set when it came from someone else.
  manifest: {
    description: string | null;
    version: string | null;
    license: string | null;
    source: string | null;
  };
  notes_dir: string;
  follow_gitignore: boolean;
  include_roots: { path: string; present: boolean }[];
  project_notes: number;
  reference_notes: number;
  skipped: number;
  skipped_dependency: number;
  skipped_by_dir: [string, number][];
  truncated: boolean;
  ambiguous_titles: { title: string; keys: string[] }[];
  reference_only_collisions: number;
  /// `null` when the server was built without semantic search, which is a
  /// different answer from "built with it and nothing embedded yet".
  embeddings: EmbeddingStatus | null;
}

export interface EmbeddingStatus {
  model: string | null;
  notes: number;
  missing_project: number;
  missing_reference: number;
}

async function json<T>(response: Response): Promise<T> {
  if (!response.ok) {
    let message = `${response.status}`;
    try {
      const body = await response.json();
      if (body?.error) message = body.error;
    } catch {
      /* not json */
    }
    throw new Error(message);
  }
  return response.json() as Promise<T>;
}

const enc = encodeURIComponent;

/** Note paths contain slashes, which must survive as path separators. */
const path = (key: string) => key.split("/").map(enc).join("/");

export const api = {
  vaults: (): Promise<VaultInfo[]> => fetch("/api/vaults").then(json<VaultInfo[]>),

  addVault: (name: string, vaultPath: string): Promise<VaultInfo> =>
    fetch("/api/vaults", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ name, path: vaultPath }),
    }).then(json<VaultInfo>),

  notes: (vault: string): Promise<NoteInfo[]> =>
    fetch(`/api/vaults/${enc(vault)}/notes`).then(json<NoteInfo[]>),

  doctor: (vault: string): Promise<DoctorReport> =>
    fetch(`/api/vaults/${enc(vault)}/doctor`).then(json<DoctorReport>),

  note: (vault: string, key: string): Promise<NoteContent> =>
    fetch(`/api/notes/${enc(vault)}/${path(key)}`).then(json<NoteContent>),

  save: (
    vault: string,
    key: string,
    content: string,
  ): Promise<{ saved: boolean; indexed: boolean }> =>
    fetch(`/api/notes/${enc(vault)}/${path(key)}`, {
      method: "PUT",
      body: content,
    }).then(json<{ saved: boolean; indexed: boolean }>),

  remove: (vault: string, key: string): Promise<{ dangling_backlinks: string[] }> =>
    fetch(`/api/notes/${enc(vault)}/${path(key)}`, { method: "DELETE" }).then(
      json<{ dangling_backlinks: string[] }>,
    ),

  links: (vault: string, key: string): Promise<NoteLinks> =>
    fetch(`/api/links/${enc(vault)}/${path(key)}`).then(json<NoteLinks>),

  search: (q: string, vault?: string, limit?: number): Promise<SearchHit[]> =>
    fetch(
      `/api/search?q=${enc(q)}${vault ? `&vault=${enc(vault)}` : ""}` +
        `${limit ? `&limit=${limit}` : ""}`,
    ).then(json<SearchHit[]>),

  graph: (vault?: string): Promise<GraphData> =>
    fetch(`/api/graph${vault ? `?vault=${enc(vault)}` : ""}`).then(json<GraphData>),
};

/** Note key for a new note created at the vault root from a title. */
export const keyForTitle = (title: string) => `${title}.md`;

/** Display title implied by a note key: the filename without `.md`. */
export const titleFromKey = (key: string) => {
  const name = key.slice(key.lastIndexOf("/") + 1);
  return name.endsWith(".md") ? name.slice(0, -3) : name;
};
