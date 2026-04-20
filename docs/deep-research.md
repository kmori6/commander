# Deep Research Processing Flow

Based on the algorithm proposed in **R. Han et al., "Deep Researcher with Test-Time Diffusion"** (arXiv:2507.16075).

---

## Overview

This implementation follows the **Test-Time Diffusion Deep Researcher (TTD-DR)** framework, which treats research report generation as a diffusion process. A preliminary draft is iteratively refined ("denoised") using dynamically retrieved external information, guided by a self-evolutionary algorithm applied to each component.

Key parameters:

- `max_revision_steps` = 3 (paper used 20)
- `n_q` = 5 (question candidates per step)
- `n_a` = 3 (answer candidates per step)

---

## Processing Flow

```mermaid
flowchart TD
    A([User Query]) --> B

    subgraph PLAN["1. Research Plan (Self-Evolutionary)"]
        B[Generate Initial Plan] --> C[Critique Plan]
        C --> D[Revise Plan]
    end

    D --> E

    subgraph DRAFT["2. Preliminary Draft"]
        E[Generate Draft\nfrom Plan & Internal Knowledge]
    end

    E --> F

    subgraph LOOP["3. Iterative Denoising Loop  ×max_revision_steps"]
        F[Generate Question Candidates\nn_q = 5] --> G[Select Best Question]
        G --> H[Web Search]
        H --> I[Generate Answer Candidates\nn_a = 3]
        I --> J[Merge Answers]
        J --> K[Denoise Draft\nIntegrate New Q&A]
        K --> L{Coverage\nSufficient?}
        L -- No --> F
        L -- Yes --> M
    end

    M --> N

    subgraph FINAL["4. Final Report (Self-Evolutionary)"]
        N[Generate Initial Report] --> O[Critique Report]
        O --> P[Revise Report]
    end

    P --> Q([Final Report])
```

---

## Component Details

### Research Plan — Self-Evolutionary

The plan is generated in three LLM calls: initial draft → critique → revision. This mirrors the paper's self-evolutionary mechanism applied to each agentic component.

### Preliminary Draft

Written from the LLM's internal knowledge only, structured around the research plan. Serves as the evolving "noisy" document that the diffusion process will refine.

### Iterative Denoising Loop

Each iteration corresponds to one diffusion step:

| Sub-step                | Description                                                                                                          |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------- |
| **Question generation** | Produce `n_q = 5` candidate questions targeting gaps in the current draft, then select the best one                  |
| **Answer retrieval**    | Search the web for the chosen question; generate `n_a = 3` candidate answers from retrieved documents and merge them |
| **Draft denoising**     | Revise the full draft by integrating the new Q&A pair; reduces "noise" (uncertainty, gaps) in the report             |
| **Exit check**          | LLM evaluates coverage of each plan section; stops early when all sections are adequately covered                    |

### Final Report — Self-Evolutionary

Applies the same critique-and-revision loop to produce a polished final output.
