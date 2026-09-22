# cafe

A terminal-based LLM frontend that doesn't know what it wants to be.

## But why?

A common, maybe tongue-in-cheek answer to "what's the best harness?" in [r/LocalLLaMA](https://www.reddit.com/r/LocalLLaMA/) is "the one you write yourself."

This is the beginning of my attempt.

And while it is surely destined to become forgotten harness #8797, my goals for it are:

* A frontend more focused on chatting, role-play, creative writing.
   * Being able to pull in character cards and lorebooks would be an eventual goal.
   * Branching/regeneration of chat history.
   * Fine-grained control of the system prompt.
* Agentic architecture with tool calling.
   * Less of a coding- or general-purpose harness.
   * I still haven't decided what kind of tools it should have, but the foundation is there.
* Backend agnostic, primarily OpenAI-compatible APIs (Chat Completion and probably stateless Responses APIs)
* A proper TUI and maybe a web UI. But I will almost certainly end up vibe coding that part of it.

Anyway, prior to this effort, I did end up evaluating quite a few (open source) harnesses.

* I know [pi](https://github.com/earendil-works/pi) is currently the new hotness in minimal harnesses, along with [deepseek-harness](https://github.com/deepseek-ai/deepseek-harness)
* [hermes](https://github.com/nousresearch/hermes-agent) was pretty big when everyone was shilling OpenClaw and its derivatives.
* And quite a few others, especially the Rust-based ones (I see you [ZeroClaw](https://github.com/zeroclaw-labs/zeroclaw) and [Goose](https://github.com/aaif-goose/goose))

But I will say that my personal favorite, and the one that I've used extensively and still use is [maki](https://github.com/tontinton/maki). Rust-based, minimal by design, extensible by Lua.

Anyway, that's all I wanted to say for now. This is just a toy, etc. etc.
