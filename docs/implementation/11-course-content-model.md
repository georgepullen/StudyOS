# Course Content Model

## Goal

Give V1 enough explicit course structure that the tutor can behave like a targeted study system for the actual maths modules, rather than an ungrounded general-purpose tutor.

## Why This Belongs In V1

Without a course content model, the app cannot reliably:

- choose prerequisite checks
- map errors to known concepts
- prioritize revision by module and topic
- surface relevant materials
- generate a useful review queue

## V1 Content Units

### Course

Top-level module grouping.

Examples:

- Matrix Algebra & Linear Models
- Probability & Statistics for Scientists

### Topic

A practical grouping inside a course.

Examples:

- matrix multiplication
- Gaussian elimination
- eigenvalues and eigenvectors
- random variables
- expectation and variance
- covariance and correlation
- simple linear regression

### Concept

Smallest unit used for scheduling, misconception tracking, and question selection.

Examples:

- “matrix multiplication is only defined when inner dimensions match”
- “determinant zero implies singular matrix”
- “variance is expectation of squared deviation from the mean”

## V1 Metadata Requirements

Each concept should support:

- stable id
- course id
- topic id
- display name
- concise description
- prerequisite concept ids
- tags
- difficulty band

## Suggested Local Source Files

```text
.studyos/
    courses/
        linear-models.toml
        probability-stats.toml
```

Each file can define:

- topics
- concepts
- prerequisite graph
- recommended question styles
- linked materials tags

## Use In Session Planning

The planner should use course metadata to:

- detect missing prerequisites
- avoid jumping into advanced topics too early
- pick transfer questions from neighboring concepts
- choose review items from the correct course context

## Use In Materials Search

Materials should be linkable by:

- course id
- topic id
- concept tags

This allows V1 to surface “relevant example from worksheet” behavior later without redesigning the metadata.

## V1 Authoring Expectations

V1 does not need a polished authoring UI.

It does need:

- editable local files
- schema validation on load
- good error messages for malformed course metadata

## Deferred

- GUI authoring tools
- automatic curriculum extraction from PDFs
- semantic prerequisite discovery
