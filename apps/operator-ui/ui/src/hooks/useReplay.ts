import { useCallback, useEffect, useState } from "react";

import type { FramePayload } from "../domain/camera";
import type { RecordedFrame, RecordedSession } from "../domain/replay";
import {
  getLatestReplayFrame,
  getRecordedSessionFrames,
  getRecordedSessions,
  selectRecordedFrame,
} from "../tauri/commands";
import { subscribeLatestReplayFrame } from "../tauri/events";

export type ReplayState = {
  error: string | null;
  frame: FramePayload | null;
  frames: RecordedFrame[];
  loading: boolean;
  refresh: () => Promise<void>;
  selectFrame: (sessionId: string, frameId: number) => Promise<void>;
  selectedSessionId: string | null;
  selectSession: (sessionId: string) => Promise<void>;
  sessions: RecordedSession[];
};

export function useReplay(): ReplayState {
  const [sessions, setSessions] = useState<RecordedSession[]>([]);
  const [frames, setFrames] = useState<RecordedFrame[]>([]);
  const [frame, setFrame] = useState<FramePayload | null>(null);
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setSessions(await getRecordedSessions());
      setError(null);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setLoading(false);
    }
  }, []);

  const selectFrame = useCallback(async (sessionId: string, frameId: number) => {
    try {
      await selectRecordedFrame(sessionId, frameId);
      setError(null);
    } catch (reason) {
      setError(String(reason));
    }
  }, []);

  const selectSession = useCallback(
    async (sessionId: string) => {
      setLoading(true);
      try {
        const nextFrames = await getRecordedSessionFrames(sessionId);
        setSelectedSessionId(sessionId);
        setFrames(nextFrames);
        setError(null);
        const latest = nextFrames[nextFrames.length - 1];
        if (latest) await selectFrame(sessionId, latest.meta.frame_id);
      } catch (reason) {
        setError(String(reason));
      } finally {
        setLoading(false);
      }
    },
    [selectFrame],
  );

  useEffect(() => {
    let active = true;
    void getRecordedSessions()
      .then((nextSessions) => {
        if (active) {
          setSessions(nextSessions);
          setError(null);
        }
      })
      .catch((reason: unknown) => {
        if (active) setError(String(reason));
      });
    void getLatestReplayFrame().then((nextFrame) => {
      if (active) setFrame(nextFrame);
    });
    let unlisten: () => void = () => undefined;
    void subscribeLatestReplayFrame((nextFrame) => {
      if (active) setFrame(nextFrame);
    }).then((cleanup) => {
      unlisten = cleanup;
    });
    return () => {
      active = false;
      unlisten();
    };
  }, []);

  return {
    error,
    frame,
    frames,
    loading,
    refresh,
    selectFrame,
    selectedSessionId,
    selectSession,
    sessions,
  };
}
