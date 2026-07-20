## Brooks CLI

The Brooks CLI is a command-line tool for using/exploring the features of the [Brooks implementation](https://github.com/cerfcast/brooks) of the ["metadata expression language" (MEL)](https://datatracker.ietf.org/doc/draft-ietf-cdni-metadata-expression-language/) and ["processing stages"](https://datatracker.ietf.org/doc/draft-ietf-cdni-processing-stages-metadata/) specifications being developed by the [CDNI working group](https://datatracker.ietf.org/group/cdni/about/) at the IETF.

More documentation will be posted here soon.

### Demo Running

There is a demonstration of this library running online at [mel.cerfca.st].

#### Demo MEL Interpreter

By `POST`ing to [https://mel.cerfca.st/serve/](https://mel.cerfca.st/serve/) with a properly JSON-formatted body, you can request evaluation of a MEL expression. The service expects JSON-formatted bodies according to the following syntax:

```json
{ "expr": "this is a MEL expression"}
```

For example, `POST`ing


```json
{ "expr": "5 + 4" }
```

will result in the evaluation of the MEL expression `5 + 4`. The service will respond with the result of the evaluation of the expression and logging output:


```json
{
  "value": "9",
  "log": {
    "msgs": [
      {
        "msg": "Evaluating binary expression",
        "location": {
          "start": 0,
          "extent": 5
        },
        "level": "Trace"
      },
      {
        "msg": "Using constant",
        "location": {
          "start": 0,
          "extent": 5
        },
        "level": "Trace"
      }
    ],
    "level": "Trace"
  }
}
```

Other pre-made queries are available 

1. [In Hoppscotch](https://hopp.sh/r/08btkDfSMnOG)
2. [In Hoppscotch](https://hopp.sh/r/aFXyScJMlP0J)
3. [In Hoppscotch](https://hopp.sh/r/gC4eSb0AW1Vo)
4. [In Hoppscotch](https://hopp.sh/r/sFZKEuy2tgwK)

#### Demo Processing Stages Interpreter

By `GET`ing [https://mel.cerfca.st/proxy/](https://mel.cerfca.st/proxy/) with a URL in the query string, you can request the demo service to proxy an HTTP request to the given URL ... after applying transformations according to a Processing Stages JSON document. The demo service is currently configured as

```json
{
    "generic-metadata-type": "MI.ClientRequestStage",
    "generic-metadata-value": {
        "match-groups": [
            {
                "generic-metadata-type": "MI.MatchGroup",
                "generic-metadata-value": {
                    "if-rule": {
                        "generic-metadata-type": "MI.StageRules",
                        "generic-metadata-value": {
                            "match": {
                                "generic-metadata-type": "MI.ExpressionMatch",
                                "generic-metadata-value": {
                                    "expression": "true"
                                }
                            },
                            "stage-metadata": {
                                "generic-metadata-type": "MI.StageMetadata",
                                "generic-metadata-value": {
                                    "generic-metadata": [],
                                    "request-transform": {
                                        "generic-metadata-type": "MI.RequestTransform",
                                        "generic-metadata-value": {
                                            "header-transform": {
                                                "generic-metadata-type": "MI.HeaderTransform",
                                                "generic-metadata-value": {
                                                    "delete": [
                                                        "delete-me"
                                                    ],
                                                    "add": [
                                                        {
                                                            "generic-metadata-type": "MI.Header",
                                                            "generic-metadata-value": {
                                                                "name": "request-is-new-expr",
                                                                "value": "req^uri^path",
                                                                "value-is-expression": true
                                                            }
                                                        },
                                                        {
                                                            "generic-metadata-type": "MI.Header",
                                                            "generic-metadata-value": {
                                                                "name": "request-is-new",
                                                                "value": "SOMETHING",
                                                                "value-is-expression": false
                                                            }
                                                        }
                                                    ]
                                                }
                                            }
                                        }
                                    },
                                    "response-transform": {
                                        "generic-metadata-type": "MI.ResponseTransform",
                                        "generic-metadata-value": {
                                            "header-transform": {
                                                "generic-metadata-type": "MI.HeaderTransform",
                                                "generic-metadata-value": {
                                                    "delete": [
                                                        "delete-me"
                                                    ],
                                                    "add": [
                                                        {
                                                            "generic-metadata-type": "MI.Header",
                                                            "generic-metadata-value": {
                                                                "name": "response-is-new-expr",
                                                                "value": "req^uri^path",
                                                                "value-is-expression": true
                                                            }
                                                        },
                                                        {
                                                            "generic-metadata-type": "MI.Header",
                                                            "generic-metadata-value": {
                                                                "name": "response-is-new",
                                                                "value": "something"
                                                            }
                                                        }
                                                    ]
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        ]
    }
}
```

If you `GET` [https://mel.cerfca.st/proxy/?http://www.example.com](https://mel.cerfca.st/proxy/?http://www.example.com), the outbound request from the proxy server to example.com will include two additional headers (and guarantee that a header named `delete-me` is not present); the response from example.com to the proxy server (and, thus, to you as the _client_) will also include two additional headers (and have the same no-`delete-me`-header guarantee as the request). 

To explore the details of processing stages interpreter, there is a [hopscotch](https://hopp.sh/r/26tlnbO6PMzn) available. You can also see details of the request/response by issuing the `GET` with `curl`:

```console
curl http://localhost:8080/proxy/?http://www.example.com -H "delete-me: DELETE" -v
```

which will direct `curl` to output the contents of the request/response. The demo instance's configuration specifies

1. the removal of the `delete-me` header from the proxied request to example.com;
2. the addition of the `request-is-new-expr` and `request-is-new` headers to the proxied request to example.com;
3. the removal of the `delete-me` header (if present) from the response from example.com; and
4. the addition of the `response-is-new-expr` and `response-is-new` headers to the proxied response from example.com.

The effects of (1) and (2) will not be visible in `curl`'s output, but the effects of (3) and (4) will:

```
...
< HTTP/1.1 200 OK
< content-length: 559
< response-is-new: something
< access-control-allow-credentials: true
< response-is-new-expr: /proxy/
< access-control-expose-headers: response-is-new-expr, content-type, response-is-new
< vary: Origin, Access-Control-Request-Method, Access-Control-Request-Headers
< content-type: text/plain; charset=utf-8
...
```