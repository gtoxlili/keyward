# FAQ

🌐 **English** · [中文](../zh/faq.md)

> Back to the [docs index](./README.md).

**Can the app read my raw key?**

No — never. It only ever sends a work intent; the key is attached inside your
Client. That's the one promise you can verify yourself (see
[Verify it yourself](./users.md#5-verify-it-yourself)).

**Can the app see my prompts?**

Yes. Keyward protects the *credential*, not the *content* — the app builds and reads
the prompts by construction. Hiding content from the app is a different, out-of-scope
problem.

**Can a malicious app burn my budget?**

Only within your limits. Custody isn't control — that's exactly why model allow-lists,
budget caps, rate limits and expiry live in the Client and run before every call.

**Isn't this just a proxy / LiteLLM?**

No. A gateway *holds* your key and forwards calls — custodial, you're trusting a
server. Keyward's Node holds nothing and literally cannot make a call without
a live Client on your side.

**What if I close the tab / stop the Client?**

All work stops immediately — the Node can't call the provider without a live
Client. For autonomous runs, keep the Client running on a box you own.

**Which providers work today?**

OpenAI Chat Completions, the OpenAI Responses API, and Anthropic Messages
(Chat-Completions also covers OpenAI-compatible providers). Gemini / tool-use / images
aren't verified yet. See [status](./README.md#status--current-limits).

---

Found a hole in the core promise? Report it privately ([SECURITY.md](../../SECURITY.md)).
