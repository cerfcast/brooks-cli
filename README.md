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

By `GET`ing to [https://mel.cerfca.st/proxy/](https://mel.cerfca.st/proxy/) with a URL in the query string, you can request the service to proxy an HTTP request to the given URL ... after applying transformations according to a Processing Stages JSON document. In particular, the demo service is currently configured as

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

For example, if you go to [https://mel.cerfca.st/proxy/?http://www.example.com](https://mel.cerfca.st/proxy/?http://www.example.com), the outbound request to example.com will include two additional headers (and guarantee that a header named `delete-me` is not present); the response from example.com will also include two additional headers (and have the same no-`delete-me`-header guarantee as the request).