import { ApiResponse, AppSettings } from '../types';

export const sendMessageToOrchestrator = async (
  prompt: string, 
  settings: AppSettings
): Promise<ApiResponse> => {
  try {
    const response = await fetch(settings.apiUrl, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        prompt,
        stream: settings.stream,
        user_alias: settings.userAlias,
        model: settings.llmModel,
        temperature: settings.llmTemperature,
        max_tokens: settings.llmMaxTokens,
        persona: settings.orchestratorPersona,
      }),
    });

    if (!response.ok) {
      throw new Error(`Backend responded with status: ${response.status}`);
    }

    const data = await response.json();
    
    // Normalize response if the backend returns a simple "thought" string instead of layers
    if (data.thought && !data.thoughts) {
      data.thoughts = [{
        id: 'default-thought',
        title: 'Orchestrator Reasoning',
        content: data.thought,
        expanded: true
      }];
    }

    return data as ApiResponse;
  } catch (error) {
    console.error("API Error:", error);
    throw error;
  }
};

export const streamMessageToOrchestrator = async function* (
  prompt: string,
  settings: AppSettings
): AsyncGenerator<string, void, unknown> {
  try {
    const response = await fetch(settings.apiUrl, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({
        prompt,
        stream: true,
        user_alias: settings.userAlias,
        model: settings.llmModel,
        temperature: settings.llmTemperature,
        max_tokens: settings.llmMaxTokens,
        persona: settings.orchestratorPersona,
      }),
    });

    if (!response.ok) {
      throw new Error(`Backend responded with status: ${response.status}`);
    }

    if (!response.body) {
      throw new Error("Response body is not readable");
    }

    const reader = response.body.getReader();
    const decoder = new TextDecoder();

    // The backend streams using SSE (`axum::response::sse::Sse`) which frames data as:
    //   data: <payload>\n\n
    // This client previously yielded raw chunks, which can look “idle” if the UI
    // expects plain tokens. Parse SSE frames and yield only the `data:` payload.
    let buffer = '';

    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });

        // Split on newlines; keep last partial line in `buffer`.
        const lines = buffer.split(/\r?\n/);
        buffer = lines.pop() ?? '';

        for (const line of lines) {
          const trimmed = line.trimEnd();
          if (!trimmed) continue;
          if (trimmed.startsWith(':')) {
            // SSE comment / keepalive
            continue;
          }
          if (trimmed.startsWith('data:')) {
            const data = trimmed.replace(/^data:\s?/, '');

            // If backend ever sends JSON payloads, try to extract a `content` field.
            // Otherwise treat `data` as plain text.
            if (data.startsWith('{') || data.startsWith('[')) {
              try {
                const parsed: any = JSON.parse(data);
                const content =
                  typeof parsed === 'string'
                    ? parsed
                    : (parsed?.content ?? parsed?.token ?? parsed?.text ?? data);
                yield String(content);
              } catch {
                yield data;
              }
            } else {
              yield data;
            }
          }
        }
      }

      // Flush any remaining buffered line (best-effort)
      const tail = buffer.trim();
      if (tail.startsWith('data:')) {
        yield tail.replace(/^data:\s?/, '');
      }
    } finally {
      reader.releaseLock();
    }
  } catch (error) {
    console.error("Stream API Error:", error);
    throw error;
  }
};
