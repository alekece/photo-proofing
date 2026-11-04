# Failed-attempts middleware

Issue: #79. Revises the approach of PR #87 after review ("Make it a true middleware").

## Intent

The failed-attempt limiter exists to stop brute-force password guessing per peer. Counting
those failures should be an explicit, reusable seam rather than a response marker that every
error path must remember to set.

## Goals

- A real actix middleware (`Transform`/`Service`, like `SameOrigin`) records failed attempts.
- The generic mechanism ("record the peer when the response status matches") is separate from
  the policy ("which statuses count"), which is declared where the routes are defined.
- `ErrorResponse` stays generic; no typed error is reintroduced to carry meaning.

## Non-goals

- Changing the limiter itself (`AttemptLimiter`, window, limits) or the `Attempt` gate.
- Limiting routes that take no credentials (`status`, `logout`, `renew`).

## Anti-goals

- Counting server faults (`5xx`) or the limiter's own `429`; that would lock out honest peers
  during an outage or under load.

## Approaches

1. **Keep the marker, centralise it.** Smallest change, but the `guess` flag and marker stay.
2. **Middleware owns the policy by inspecting the response.** No flag or marker. Needs a
   typed error to distinguish a guess, which the owner rejected.
3. **Count in the handler.** What the PR removed; not viable for extractor rejections.

Chosen: a status-based variant of 2. The middleware decides from the response status through a
configurable predicate, so no typed error is needed. The owner picked this.

## Design

- `FailedAttempts<F>` with `F: Fn(StatusCode) -> bool + 'static` is a `Transform` and
  `Service` pair modelled on `SameOrigin`. The predicate is held in an `Rc<F>` because
  `new_transform` takes `&self` and the `'static` future returned by `call` runs the check
  after the inner service responds; `Rc` avoids a `Clone` bound (workers are single-threaded).
- After the inner service responds, if the predicate accepts the response status, it calls
  `ApiState::record_failure` for the request's peer.
- `Default` counts every `4xx` except `429`; `5xx` never counts. `.counting(predicate)`
  replaces the rule with any function or closure.
- `routes/auth.rs` wraps `login` and `setup` only, with the default.
- `FailedGuess`, `ErrorResponse::guess`, `AuthError::is_guess` and the marking in
  `error_response` are removed. `/auth` keeps using the API-wide `JsonConfig`.
- `Attempt` remains the gate that rejects over-limit peers with `429`.

## Behaviour change

`413`, `415` and syntax `400` responses on `login`/`setup` now count as failed attempts.
Issue #79's acceptance criteria excluded them; the issue text and PR description are updated
to match. The cost is that a buggy client can lock itself out; an attacker gains nothing.

## Left to the builder

Module naming and file layout, doc comments, and tests: the default rule (401/422 count,
429/5xx do not), a custom predicate, and that unprotected `/auth` routes never count.
