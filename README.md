# inqui

*A flexible toolkit for building on-demand, memoizing query systems.*

The key idea of "query systems" is to have *queries* that compute an information
from the *inputs*. The program then uses queries, but only those that are needed
for the specific task (*demand-driven*). The outputs of queries are cached so
they are available without recomputation in the future, until the inputs they
depend on change (*memoization*).

## Credits

The API and a lot of implementation details was inspired from
[salsa](https://github.com/salsa-rs/salsa) framework, which has more features
and documentation, a lot more thought was put into its design and a lot more
effort was put into its implementation. Essentially the only reason for inqui's
existence is the lack of support for dynamic queries, which is on their
[wishlist](https://github.com/salsa-rs/salsa/issues/23), although having a *Far
future* milestone.

## Documentation

... does not exist. But you should find some information and inspiration in
[examples](examples/) (especially heavily-commented
[hello_world](examples/hello_world.rs)) or the [integration
tests](tests/common.rs).

## License

Dual-licensed under [MIT](LICENSE) and [UNLICENSE](UNLICENSE). Feel free to use
it, contribute or spread the word.
