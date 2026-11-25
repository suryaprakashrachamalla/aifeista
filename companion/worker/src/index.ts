/**
 * Carnival Companion Proxy Worker
 *
 * Proxies requests to LLM APIs (Groq, OpenRouter, NVIDIA Nemotron, Anthropic)
 * so the app never ships with raw API keys.
 * Voice Transcription (S2T) and Speech (T2S) run 100% locally via Apple's native frameworks.
 *
 * Routes:
 *   POST /chat → Chat completion with vision
 */

interface Env {
  // LLM Provider keys
  GROQ_API_KEY?: string;
  OPENROUTER_API_KEY?: string;
  NVIDIA_API_KEY?: string;
  ANTHROPIC_API_KEY?: string;

  // Provider configuration
  LLM_PROVIDER?: "groq" | "openrouter" | "nvidia" | "anthropic";
  GROQ_MODEL?: string;
  OPENROUTER_MODEL?: string;
  NVIDIA_MODEL?: string;
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);

    // Support CORS preflight for local development
    if (request.method === "OPTIONS") {
      return new Response(null, {
        headers: {
          "access-control-allow-origin": "*",
          "access-control-allow-methods": "POST, OPTIONS",
          "access-control-allow-headers": "Content-Type, Authorization, x-llm-provider",
        },
      });
    }

    if (request.method !== "POST") {
      return new Response("Method not allowed", { status: 405 });
    }

    try {
      if (url.pathname === "/chat") {
        return await handleChat(request, env);
      }
    } catch (error) {
      console.error(`[${url.pathname}] Unhandled error:`, error);
      return new Response(
        JSON.stringify({ error: String(error) }),
        { status: 500, headers: { "content-type": "application/json" } }
      );
    }

    return new Response("Not found", { status: 404 });
  },
};

/**
 * Resolves which provider to use based on request header, environment variable,
 * or automatic detection based on configured API keys.
 */
function resolveProvider(request: Request, env: Env): "openrouter" | "nvidia" | "groq" | "anthropic" {
  const headerProvider = request.headers.get("x-llm-provider")?.toLowerCase();
  if (headerProvider === "openrouter" || headerProvider === "nvidia" || headerProvider === "groq" || headerProvider === "anthropic") {
    return headerProvider;
  }

  if (env.LLM_PROVIDER) {
    const configured = env.LLM_PROVIDER.toLowerCase();
    if (configured === "openrouter" || configured === "nvidia" || configured === "groq" || configured === "anthropic") {
      return configured;
    }
  }

  // Automatic detection
  if (env.OPENROUTER_API_KEY) return "openrouter";
  if (env.GROQ_API_KEY) return "groq";
  if (env.NVIDIA_API_KEY) return "nvidia";
  if (env.ANTHROPIC_API_KEY) return "anthropic";

  // Default to openrouter if nothing set
  return "openrouter";
}

async function handleChat(request: Request, env: Env): Promise<Response> {
  const provider = resolveProvider(request, env);
  const rawBody = await request.text();

  let body: any;
  try {
    body = JSON.parse(rawBody);
  } catch {
    return new Response(JSON.stringify({ error: "Invalid JSON request body" }), {
      status: 400,
      headers: { "content-type": "application/json" },
    });
  }

  // 1. Anthropic Direct Handler
  if (provider === "anthropic") {
    if (!env.ANTHROPIC_API_KEY) {
      return new Response(JSON.stringify({ error: "ANTHROPIC_API_KEY is not configured" }), {
        status: 400,
        headers: { "content-type": "application/json" },
      });
    }

    const response = await fetch("https://api.anthropic.com/v1/messages", {
      method: "POST",
      headers: {
        "x-api-key": env.ANTHROPIC_API_KEY,
        "anthropic-version": "2023-06-01",
        "content-type": "application/json",
      },
      body: rawBody,
    });

    if (!response.ok) {
      const errorBody = await response.text();
      console.error(`[/chat] Anthropic error ${response.status}: ${errorBody}`);
      return new Response(errorBody, {
        status: response.status,
        headers: { "content-type": "application/json" },
      });
    }

    return new Response(response.body, {
      status: response.status,
      headers: {
        "content-type": response.headers.get("content-type") || "text/event-stream",
        "cache-control": "no-cache",
      },
    });
  }

  // 2. OpenAI-compatible Providers: OpenRouter, NVIDIA Nemotron, Groq
  let endpoint = "";
  let apiKey = "";
  let defaultModel = "";
  const extraHeaders: Record<string, string> = {};

  switch (provider) {
    case "openrouter":
      endpoint = "https://openrouter.ai/api/v1/chat/completions";
      apiKey = env.OPENROUTER_API_KEY || "";
      defaultModel = env.OPENROUTER_MODEL || "meta-llama/llama-3.2-11b-vision-instruct";
      extraHeaders["HTTP-Referer"] = "https://carnival.local";
      extraHeaders["X-Title"] = "Carnival Companion";
      break;

    case "nvidia":
      endpoint = "https://integrate.api.nvidia.com/v1/chat/completions";
      apiKey = env.NVIDIA_API_KEY || "";
      defaultModel = env.NVIDIA_MODEL || "nvidia/nemotron-3.5-super-vl-120b-a12b";
      break;

    case "groq":
      endpoint = "https://api.groq.com/openai/v1/chat/completions";
      apiKey = env.GROQ_API_KEY || "";
      defaultModel = env.GROQ_MODEL || "qwen/qwen3.8-27b";
      break;
  }

  if (!apiKey) {
    return new Response(
      JSON.stringify({
        error: `API key for provider '${provider}' is not set. Please set ${provider.toUpperCase()}_API_KEY.`,
      }),
      { status: 400, headers: { "content-type": "application/json" } }
    );
  }

  // Model selection: if request passed a generic claude name or no model, use provider's vision default
  let selectedModel = body.model;
  if (!selectedModel || selectedModel.startsWith("claude-")) {
    selectedModel = defaultModel;
  }

  // Convert Anthropic-style messages to OpenAI-compatible messages
  const openAiMessages: any[] = [];

  // Add system message if present
  if (body.system) {
    openAiMessages.push({
      role: "system",
      content: body.system,
    });
  }

  // Convert conversation messages
  if (Array.isArray(body.messages)) {
    for (const msg of body.messages) {
      if (typeof msg.content === "string") {
        openAiMessages.push({
          role: msg.role,
          content: msg.content,
        });
        continue;
      }

      if (Array.isArray(msg.content)) {
        const convertedParts: any[] = [];
        for (const part of msg.content) {
          if (part.type === "image" && part.source?.type === "base64") {
            const mediaType = part.source.media_type || "image/jpeg";
            const base64Data = part.source.data;
            convertedParts.push({
              type: "image_url",
              image_url: {
                url: `data:${mediaType};base64,${base64Data}`,
              },
            });
          } else if (part.type === "text") {
            convertedParts.push({
              type: "text",
              text: part.text,
            });
          } else {
            convertedParts.push(part);
          }
        }

        openAiMessages.push({
          role: msg.role,
          content: convertedParts,
        });
      }
    }
  }

  const openAiPayload = {
    model: selectedModel,
    messages: openAiMessages,
    max_tokens: body.max_tokens || 1024,
    stream: true,
  };

  const response = await fetch(endpoint, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
      ...extraHeaders,
    },
    body: JSON.stringify(openAiPayload),
  });

  if (!response.ok) {
    const errorBody = await response.text();
    console.error(`[/chat] ${provider} API error ${response.status}: ${errorBody}`);
    return new Response(errorBody, {
      status: response.status,
      headers: { "content-type": "application/json" },
    });
  }

  if (!response.body) {
    return new Response(JSON.stringify({ error: "Empty response body from provider" }), {
      status: 500,
      headers: { "content-type": "application/json" },
    });
  }

  // Convert OpenAI streaming SSE format into Anthropic SSE format on the fly
  // so the macOS client's existing SSE parser receives text chunks seamlessly.
  const transformedStream = transformOpenAIToAnthropicSSE(response.body);

  return new Response(transformedStream, {
    status: 200,
    headers: {
      "content-type": "text/event-stream",
      "cache-control": "no-cache",
    },
  });
}

/**
 * Transforms an OpenAI-compatible SSE stream (`choices[0].delta.content`)
 * into Anthropic-compatible SSE (`content_block_delta` with `text_delta`).
 */
function transformOpenAIToAnthropicSSE(stream: ReadableStream<Uint8Array>): ReadableStream<Uint8Array> {
  const reader = stream.getReader();
  const decoder = new TextDecoder();
  const encoder = new TextEncoder();
  let buffer = "";

  return new ReadableStream({
    async pull(controller) {
      while (true) {
        const { done, value } = await reader.read();
        if (done) {
          controller.enqueue(encoder.encode("data: [DONE]\n\n"));
          controller.close();
          return;
        }

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split("\n");
        buffer = lines.pop() || "";

        for (const line of lines) {
          const trimmed = line.trim();
          if (!trimmed.startsWith("data:")) continue;

          const dataPayload = trimmed.replace(/^data:\s*/, "");
          if (dataPayload === "[DONE]") {
            controller.enqueue(encoder.encode("data: [DONE]\n\n"));
            continue;
          }

          try {
            const parsed = JSON.parse(dataPayload);
            const delta = parsed.choices?.[0]?.delta;
            // Prefer content (the final response). If delta.content is null/undefined during reasoning, skip
            // so internal reasoning tokens aren't spoken aloud by ElevenLabs TTS.
            const textChunk = delta?.content;
            if (typeof textChunk === "string" && textChunk.length > 0) {
              const anthropicEvent = {
                type: "content_block_delta",
                index: 0,
                delta: {
                  type: "text_delta",
                  text: textChunk,
                },
              };
              controller.enqueue(encoder.encode(`data: ${JSON.stringify(anthropicEvent)}\n\n`));
            }
          } catch {
            // Ignore non-json lines
          }
        }
      }
    },
    cancel() {
      reader.cancel();
    },
  });
}
