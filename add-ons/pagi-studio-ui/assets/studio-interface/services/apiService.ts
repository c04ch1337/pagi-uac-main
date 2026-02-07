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
    const contentType = response.headers.get('content-type') ?? '';
    const isSSE = contentType.includes('text/event-stream');

    try {
      if (isSSE) {
        // SSE mode: parse `data: <payload>\n\n` frames.
        let buffer = '';
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;

          buffer += decoder.decode(value, { stream: true });
          const lines = buffer.split(/\r?\n/);
          buffer = lines.pop() ?? '';

          for (const line of lines) {
            const trimmed = line.trimEnd();
            if (!trimmed || trimmed.startsWith(':')) continue;
            if (trimmed.startsWith('data:')) {
              const data = trimmed.replace(/^data:\s?/, '');
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
        // Flush remaining SSE buffer
        const tail = (buffer ?? '').trim();
        if (tail.startsWith('data:')) {
          yield tail.replace(/^data:\s?/, '');
        }
      } else {
        // Plain-text streaming mode (gateway default: text/plain chunks).
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          const chunk = decoder.decode(value, { stream: true });
          if (chunk) yield chunk;
        }
      }
    } finally {
      reader.releaseLock();
    }
  } catch (error) {
    console.error("Stream API Error:", error);
    throw error;
  }
};
