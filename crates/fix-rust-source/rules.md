# autofix rules:

## remove code comments:

justification:
- often llm code comments encode justification that doesn't match a real requirement, so it can mislead future agents about project goals
- often code comments don't get updated when functionality changes, misleading future agents
- agents are pretty good at figuring out what code does, so the code comments could just be a waste of tokens

counter:
- maybe code comments could help agents to figure out what functions do enough to counter their token cost
- maybe code comments could make agent code generation better
- for human coding, doc comments would be nice to have at least

empirical:
- agent performance impact has not been measured

## one test per file:

justification:
- having tests in the main file makes it larger
- agents finding one test may read more than they need, wasting tokens. one test per file makes it so an agent is less likely to be distracted by test file content in reads, and can easily read a full test file when needed.

counter:
- this is not common convention that agents were trained on, so it could reduce their performance
- maybe it's useful to have tests in-context sometimes as examples of library usage, and agents might not seek them out when they're not there
- maybe agents will spend more tokens writing tests because of the more complicated test addition procedures
- at least once I have seen an agent forget to commit files that were added by verify - the mention in AGENTS.md seems to have solved this

empirical:
- agent performance impact has not been measured

## a.rs instead of a/mod.rs:

justification:
- personal stylistic preference: it looks better in vscode. a/mod.rs is just kind of randomly in the middle of a folder with other 'a' files, whereas a.rs is visibly the parent to 'a'
- maybe it could hypothetically save tokens in folder listing?

counter:
- maybe mod.rs is more common and maybe agents could perform marginally better because it is closer to their training data
- I rarely look at the code anyway so why do I care

empirical:
- I enjoy it when I do look at the code
- agent performance impact has not been measured
