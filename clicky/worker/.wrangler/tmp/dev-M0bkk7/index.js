var __defProp = Object.defineProperty;
var __name = (target, value) => __defProp(target, "name", { value, configurable: true });

// .wrangler/tmp/bundle-D1Su5H/checked-fetch.js
var urls = /* @__PURE__ */ new Set();
function checkURL(request, init) {
  const url = request instanceof URL ? request : new URL(
    (typeof request === "string" ? new Request(request, init) : request).url
  );
  if (url.port && url.port !== "443" && url.protocol === "https:") {
    if (!urls.has(url.toString())) {
      urls.add(url.toString());
      console.warn(
        `WARNING: known issue with \`fetch()\` requests to custom HTTPS ports in published Workers:
 - ${url.toString()} - the custom port will be ignored when the Worker is published using the \`wrangler deploy\` command.
`
      );
    }
  }
}
__name(checkURL, "checkURL");
globalThis.fetch = new Proxy(globalThis.fetch, {
  apply(target, thisArg, argArray) {
    const [request, init] = argArray;
    checkURL(request, init);
    return Reflect.apply(target, thisArg, argArray);
  }
});

// .wrangler/tmp/bundle-D1Su5H/strip-cf-connecting-ip-header.js
function stripCfConnectingIPHeader(input, init) {
  const request = new Request(input, init);
  request.headers.delete("CF-Connecting-IP");
  return request;
}
__name(stripCfConnectingIPHeader, "stripCfConnectingIPHeader");
globalThis.fetch = new Proxy(globalThis.fetch, {
  apply(target, thisArg, argArray) {
    return Reflect.apply(target, thisArg, [
      stripCfConnectingIPHeader.apply(null, argArray)
    ]);
  }
});

// src/index.ts
var src_default = {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (request.method === "OPTIONS") {
      return new Response(null, {
        headers: {
          "access-control-allow-origin": "*",
          "access-control-allow-methods": "POST, OPTIONS",
          "access-control-allow-headers": "Content-Type, Authorization, x-llm-provider"
        }
      });
    }
    if (request.method !== "POST") {
      return new Response("Method not allowed", { status: 405 });
    }
    try {
      if (url.pathname === "/chat") {
        return await handleChat(request, env);
      }
      if (url.pathname === "/tts") {
        return await handleTTS(request, env);
      }
      if (url.pathname === "/transcribe-token") {
        return await handleTranscribeToken(env);
      }
    } catch (error) {
      console.error(`[${url.pathname}] Unhandled error:`, error);
      return new Response(
        JSON.stringify({ error: String(error) }),
        { status: 500, headers: { "content-type": "application/json" } }
      );
    }
    return new Response("Not found", { status: 404 });
  }
};
function resolveProvider(request, env) {
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
  if (env.OPENROUTER_API_KEY)
    return "openrouter";
  if (env.GROQ_API_KEY)
    return "groq";
  if (env.NVIDIA_API_KEY)
    return "nvidia";
  if (env.ANTHROPIC_API_KEY)
    return "anthropic";
  return "openrouter";
}
__name(resolveProvider, "resolveProvider");
async function handleChat(request, env) {
  const provider = resolveProvider(request, env);
  const rawBody = await request.text();
  let body;
  try {
    body = JSON.parse(rawBody);
  } catch {
    return new Response(JSON.stringify({ error: "Invalid JSON request body" }), {
      status: 400,
      headers: { "content-type": "application/json" }
    });
  }
  if (provider === "anthropic") {
    if (!env.ANTHROPIC_API_KEY) {
      return new Response(JSON.stringify({ error: "ANTHROPIC_API_KEY is not configured" }), {
        status: 400,
        headers: { "content-type": "application/json" }
      });
    }
    const response2 = await fetch("https://api.anthropic.com/v1/messages", {
      method: "POST",
      headers: {
        "x-api-key": env.ANTHROPIC_API_KEY,
        "anthropic-version": "2023-06-01",
        "content-type": "application/json"
      },
      body: rawBody
    });
    if (!response2.ok) {
      const errorBody = await response2.text();
      console.error(`[/chat] Anthropic error ${response2.status}: ${errorBody}`);
      return new Response(errorBody, {
        status: response2.status,
        headers: { "content-type": "application/json" }
      });
    }
    return new Response(response2.body, {
      status: response2.status,
      headers: {
        "content-type": response2.headers.get("content-type") || "text/event-stream",
        "cache-control": "no-cache"
      }
    });
  }
  let endpoint = "";
  let apiKey = "";
  let defaultModel = "";
  const extraHeaders = {};
  switch (provider) {
    case "openrouter":
      endpoint = "https://openrouter.ai/api/v1/chat/completions";
      apiKey = env.OPENROUTER_API_KEY || "";
      defaultModel = env.OPENROUTER_MODEL || "meta-llama/llama-3.2-11b-vision-instruct";
      extraHeaders["HTTP-Referer"] = "https://heyclicky.com";
      extraHeaders["X-Title"] = "Clicky";
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
        error: `API key for provider '${provider}' is not set. Please set ${provider.toUpperCase()}_API_KEY.`
      }),
      { status: 400, headers: { "content-type": "application/json" } }
    );
  }
  let selectedModel = body.model;
  if (!selectedModel || selectedModel.startsWith("claude-")) {
    selectedModel = defaultModel;
  }
  const openAiMessages = [];
  if (body.system) {
    openAiMessages.push({
      role: "system",
      content: body.system
    });
  }
  if (Array.isArray(body.messages)) {
    for (const msg of body.messages) {
      if (typeof msg.content === "string") {
        openAiMessages.push({
          role: msg.role,
          content: msg.content
        });
        continue;
      }
      if (Array.isArray(msg.content)) {
        const convertedParts = [];
        for (const part of msg.content) {
          if (part.type === "image" && part.source?.type === "base64") {
            const mediaType = part.source.media_type || "image/jpeg";
            const base64Data = part.source.data;
            convertedParts.push({
              type: "image_url",
              image_url: {
                url: `data:${mediaType};base64,${base64Data}`
              }
            });
          } else if (part.type === "text") {
            convertedParts.push({
              type: "text",
              text: part.text
            });
          } else {
            convertedParts.push(part);
          }
        }
        openAiMessages.push({
          role: msg.role,
          content: convertedParts
        });
      }
    }
  }
  const openAiPayload = {
    model: selectedModel,
    messages: openAiMessages,
    max_tokens: body.max_tokens || 1024,
    stream: true
  };
  const response = await fetch(endpoint, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${apiKey}`,
      "Content-Type": "application/json",
      ...extraHeaders
    },
    body: JSON.stringify(openAiPayload)
  });
  if (!response.ok) {
    const errorBody = await response.text();
    console.error(`[/chat] ${provider} API error ${response.status}: ${errorBody}`);
    return new Response(errorBody, {
      status: response.status,
      headers: { "content-type": "application/json" }
    });
  }
  if (!response.body) {
    return new Response(JSON.stringify({ error: "Empty response body from provider" }), {
      status: 500,
      headers: { "content-type": "application/json" }
    });
  }
  const transformedStream = transformOpenAIToAnthropicSSE(response.body);
  return new Response(transformedStream, {
    status: 200,
    headers: {
      "content-type": "text/event-stream",
      "cache-control": "no-cache"
    }
  });
}
__name(handleChat, "handleChat");
function transformOpenAIToAnthropicSSE(stream) {
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
          if (!trimmed.startsWith("data:"))
            continue;
          const dataPayload = trimmed.replace(/^data:\s*/, "");
          if (dataPayload === "[DONE]") {
            controller.enqueue(encoder.encode("data: [DONE]\n\n"));
            continue;
          }
          try {
            const parsed = JSON.parse(dataPayload);
            const delta = parsed.choices?.[0]?.delta;
            const textChunk = delta?.content;
            if (typeof textChunk === "string" && textChunk.length > 0) {
              const anthropicEvent = {
                type: "content_block_delta",
                index: 0,
                delta: {
                  type: "text_delta",
                  text: textChunk
                }
              };
              controller.enqueue(encoder.encode(`data: ${JSON.stringify(anthropicEvent)}

`));
            }
          } catch {
          }
        }
      }
    },
    cancel() {
      reader.cancel();
    }
  });
}
__name(transformOpenAIToAnthropicSSE, "transformOpenAIToAnthropicSSE");
async function handleTranscribeToken(env) {
  const response = await fetch(
    "https://streaming.assemblyai.com/v3/token?expires_in_seconds=480",
    {
      method: "GET",
      headers: {
        authorization: env.ASSEMBLYAI_API_KEY
      }
    }
  );
  if (!response.ok) {
    const errorBody = await response.text();
    console.error(`[/transcribe-token] AssemblyAI token error ${response.status}: ${errorBody}`);
    return new Response(errorBody, {
      status: response.status,
      headers: { "content-type": "application/json" }
    });
  }
  const data = await response.text();
  return new Response(data, {
    status: 200,
    headers: { "content-type": "application/json" }
  });
}
__name(handleTranscribeToken, "handleTranscribeToken");
async function handleTTS(request, env) {
  const body = await request.text();
  const voiceId = env.ELEVENLABS_VOICE_ID || "kPzsL2i3teMYv0FxEYQ6";
  const response = await fetch(
    `https://api.elevenlabs.io/v1/text-to-speech/${voiceId}`,
    {
      method: "POST",
      headers: {
        "xi-api-key": env.ELEVENLABS_API_KEY,
        "content-type": "application/json",
        accept: "audio/mpeg"
      },
      body
    }
  );
  if (!response.ok) {
    const errorBody = await response.text();
    console.error(`[/tts] ElevenLabs API error ${response.status}: ${errorBody}`);
    return new Response(errorBody, {
      status: response.status,
      headers: { "content-type": "application/json" }
    });
  }
  return new Response(response.body, {
    status: response.status,
    headers: {
      "content-type": response.headers.get("content-type") || "audio/mpeg"
    }
  });
}
__name(handleTTS, "handleTTS");

// node_modules/wrangler/templates/middleware/middleware-ensure-req-body-drained.ts
var drainBody = /* @__PURE__ */ __name(async (request, env, _ctx, middlewareCtx) => {
  try {
    return await middlewareCtx.next(request, env);
  } finally {
    try {
      if (request.body !== null && !request.bodyUsed) {
        const reader = request.body.getReader();
        while (!(await reader.read()).done) {
        }
      }
    } catch (e) {
      console.error("Failed to drain the unused request body.", e);
    }
  }
}, "drainBody");
var middleware_ensure_req_body_drained_default = drainBody;

// node_modules/wrangler/templates/middleware/middleware-miniflare3-json-error.ts
function reduceError(e) {
  return {
    name: e?.name,
    message: e?.message ?? String(e),
    stack: e?.stack,
    cause: e?.cause === void 0 ? void 0 : reduceError(e.cause)
  };
}
__name(reduceError, "reduceError");
var jsonError = /* @__PURE__ */ __name(async (request, env, _ctx, middlewareCtx) => {
  try {
    return await middlewareCtx.next(request, env);
  } catch (e) {
    const error = reduceError(e);
    return Response.json(error, {
      status: 500,
      headers: { "MF-Experimental-Error-Stack": "true" }
    });
  }
}, "jsonError");
var middleware_miniflare3_json_error_default = jsonError;

// .wrangler/tmp/bundle-D1Su5H/middleware-insertion-facade.js
var __INTERNAL_WRANGLER_MIDDLEWARE__ = [
  middleware_ensure_req_body_drained_default,
  middleware_miniflare3_json_error_default
];
var middleware_insertion_facade_default = src_default;

// node_modules/wrangler/templates/middleware/common.ts
var __facade_middleware__ = [];
function __facade_register__(...args) {
  __facade_middleware__.push(...args.flat());
}
__name(__facade_register__, "__facade_register__");
function __facade_invokeChain__(request, env, ctx, dispatch, middlewareChain) {
  const [head, ...tail] = middlewareChain;
  const middlewareCtx = {
    dispatch,
    next(newRequest, newEnv) {
      return __facade_invokeChain__(newRequest, newEnv, ctx, dispatch, tail);
    }
  };
  return head(request, env, ctx, middlewareCtx);
}
__name(__facade_invokeChain__, "__facade_invokeChain__");
function __facade_invoke__(request, env, ctx, dispatch, finalMiddleware) {
  return __facade_invokeChain__(request, env, ctx, dispatch, [
    ...__facade_middleware__,
    finalMiddleware
  ]);
}
__name(__facade_invoke__, "__facade_invoke__");

// .wrangler/tmp/bundle-D1Su5H/middleware-loader.entry.ts
var __Facade_ScheduledController__ = class {
  constructor(scheduledTime, cron, noRetry) {
    this.scheduledTime = scheduledTime;
    this.cron = cron;
    this.#noRetry = noRetry;
  }
  #noRetry;
  noRetry() {
    if (!(this instanceof __Facade_ScheduledController__)) {
      throw new TypeError("Illegal invocation");
    }
    this.#noRetry();
  }
};
__name(__Facade_ScheduledController__, "__Facade_ScheduledController__");
function wrapExportedHandler(worker) {
  if (__INTERNAL_WRANGLER_MIDDLEWARE__ === void 0 || __INTERNAL_WRANGLER_MIDDLEWARE__.length === 0) {
    return worker;
  }
  for (const middleware of __INTERNAL_WRANGLER_MIDDLEWARE__) {
    __facade_register__(middleware);
  }
  const fetchDispatcher = /* @__PURE__ */ __name(function(request, env, ctx) {
    if (worker.fetch === void 0) {
      throw new Error("Handler does not export a fetch() function.");
    }
    return worker.fetch(request, env, ctx);
  }, "fetchDispatcher");
  return {
    ...worker,
    fetch(request, env, ctx) {
      const dispatcher = /* @__PURE__ */ __name(function(type, init) {
        if (type === "scheduled" && worker.scheduled !== void 0) {
          const controller = new __Facade_ScheduledController__(
            Date.now(),
            init.cron ?? "",
            () => {
            }
          );
          return worker.scheduled(controller, env, ctx);
        }
      }, "dispatcher");
      return __facade_invoke__(request, env, ctx, dispatcher, fetchDispatcher);
    }
  };
}
__name(wrapExportedHandler, "wrapExportedHandler");
function wrapWorkerEntrypoint(klass) {
  if (__INTERNAL_WRANGLER_MIDDLEWARE__ === void 0 || __INTERNAL_WRANGLER_MIDDLEWARE__.length === 0) {
    return klass;
  }
  for (const middleware of __INTERNAL_WRANGLER_MIDDLEWARE__) {
    __facade_register__(middleware);
  }
  return class extends klass {
    #fetchDispatcher = (request, env, ctx) => {
      this.env = env;
      this.ctx = ctx;
      if (super.fetch === void 0) {
        throw new Error("Entrypoint class does not define a fetch() function.");
      }
      return super.fetch(request);
    };
    #dispatcher = (type, init) => {
      if (type === "scheduled" && super.scheduled !== void 0) {
        const controller = new __Facade_ScheduledController__(
          Date.now(),
          init.cron ?? "",
          () => {
          }
        );
        return super.scheduled(controller);
      }
    };
    fetch(request) {
      return __facade_invoke__(
        request,
        this.env,
        this.ctx,
        this.#dispatcher,
        this.#fetchDispatcher
      );
    }
  };
}
__name(wrapWorkerEntrypoint, "wrapWorkerEntrypoint");
var WRAPPED_ENTRY;
if (typeof middleware_insertion_facade_default === "object") {
  WRAPPED_ENTRY = wrapExportedHandler(middleware_insertion_facade_default);
} else if (typeof middleware_insertion_facade_default === "function") {
  WRAPPED_ENTRY = wrapWorkerEntrypoint(middleware_insertion_facade_default);
}
var middleware_loader_entry_default = WRAPPED_ENTRY;
export {
  __INTERNAL_WRANGLER_MIDDLEWARE__,
  middleware_loader_entry_default as default
};
//# sourceMappingURL=index.js.map
