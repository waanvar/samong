export interface ChangeEvent {
  vault: string;
  indexed: number;
  removed: number;
}

/** Subscribe to server change events with automatic reconnect.
 *  Returns a cleanup function. */
export function listenChanges(onEvent: (event: ChangeEvent) => void): () => void {
  let socket: WebSocket | null = null;
  let closed = false;
  let retry = 1000;

  const connect = () => {
    if (closed) return;
    const proto = location.protocol === "https:" ? "wss" : "ws";
    socket = new WebSocket(`${proto}://${location.host}/ws`);
    socket.onmessage = (msg) => {
      try {
        onEvent(JSON.parse(msg.data as string) as ChangeEvent);
      } catch {
        /* ignore malformed frames */
      }
    };
    socket.onopen = () => {
      retry = 1000;
    };
    socket.onclose = () => {
      if (!closed) setTimeout(connect, (retry = Math.min(retry * 2, 15000)));
    };
  };

  connect();
  return () => {
    closed = true;
    socket?.close();
  };
}
