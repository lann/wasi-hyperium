```console
$ ./build-and-serve.sh
...
Serving HTTP on http://0.0.0.0:8080/
```

```console
$ curl localhost:8080
Hello, WASI
$ curl localhost:8080/echo -d echo...
echo...
