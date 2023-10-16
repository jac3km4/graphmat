# graphmat
Graphmat is a tool that can compute an edit sequence between call graphs of two executable
programs. Due to it's choice of algorithms it can process massive call graphs within milliseconds. It's primary purpose is to aid in reverse engineering of programs by automatically
generating a mapping of function addresses between two versions of a program.

## how
- the core of the application is based on the algorithm proposed in the
  [Error-tolerant graph matching in linear computational cost using an initial small partial matching paper](https://www.sciencedirect.com/science/article/abs/pii/S0167865518301235) (see [belief_prop.rs](src/belief_prop.rs))
  - this algorithm accepts an initial partial mapping between vertices, which can make it more accurate by giving it multiple starting points within the call graph - this mapping could for example be obtained by searching for IDA-style patterns in the binary or by finding load instructions for specific string constants
  - the linear computational cost makes this approach very attractive, in practice it is multiple orders of magnitude faster than conventional approaches of comparing executable files

- functions ([stars](https://en.wikipedia.org/wiki/Star_%28graph_theory%29)) are compared two at a time using Levenshetin distance between opcodes of its instructions (see [match_star.rs](src/match_star.rs))
- the comparison then proceeds to use multiple heuristics to label the callees of each function and then compare and find an optimal mapping between them (see [heuristics.rs](src/heuristics.rs))

## usage
```bash
Usage: cli.exe --first <FIRST> --second <SECOND> --output <OUTPUT>

Options:
  -f, --first <FIRST>    The first object file to compare
  -s, --second <SECOND>  The second object file to compare
  -o, --output <OUTPUT>  The path to write the mapping to as a CSV file
  -h, --help             Print help
  -V, --version          Print version
```
