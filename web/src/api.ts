export interface VaultInfo {
  name: string;
  path: string;
}

export interface SearchHit {
  vault: string;
  title: string;
  snippet: string;
}

export interface NoteLinks {
  forward: string[];
  backlinks: string[];
  cross_vault_backlinks: string[];
}

export interface GraphData {
  nodes: string[];
  edges: { from: string; to: string }[];
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

export const api = {
  vaults: (): Promise<VaultInfo[]> => fetch("/api/vaults").then(json<VaultInfo[]>),

  notes: (vault: string): Promise<string[]> =>
    fetch(`/api/vaults/${enc(vault)}/notes`).then(json<string[]>),

  note: (vault: string, title: string): Promise<{ title: string; content: string }> =>
    fetch(`/api/notes/${enc(vault)}/${enc(title)}`).then(
      json<{ title: string; content: string }>,
    ),

  save: (vault: string, title: string, content: string): Promise<unknown> =>
    fetch(`/api/notes/${enc(vault)}/${enc(title)}`, {
      method: "PUT",
      body: content,
    }).then(json),

  remove: (vault: string, title: string): Promise<{ dangling_backlinks: string[] }> =>
    fetch(`/api/notes/${enc(vault)}/${enc(title)}`, { method: "DELETE" }).then(
      json<{ dangling_backlinks: string[] }>,
    ),

  links: (vault: string, title: string): Promise<NoteLinks> =>
    fetch(`/api/notes/${enc(vault)}/${enc(title)}/links`).then(json<NoteLinks>),

  search: (q: string, vault?: string): Promise<SearchHit[]> =>
    fetch(`/api/search?q=${enc(q)}${vault ? `&vault=${enc(vault)}` : ""}`).then(
      json<SearchHit[]>,
    ),

  graph: (vault?: string): Promise<GraphData> =>
    fetch(`/api/graph${vault ? `?vault=${enc(vault)}` : ""}`).then(json<GraphData>),
};
