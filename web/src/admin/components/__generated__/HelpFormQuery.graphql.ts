/**
 * @generated SignedSource<<f5a743e13052f52c551ef7a2b1d6f315>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type HelpFormQuery$variables = Record<PropertyKey, never>;
export type HelpFormQuery$data = {
  readonly feedbackAvailable: boolean;
  readonly user: {
    readonly email: string;
  };
};
export type HelpFormQuery = {
  response: HelpFormQuery$data;
  variables: HelpFormQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "feedbackAvailable",
  "storageKey": null
},
v1 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "email",
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": [],
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "HelpFormQuery",
    "selections": [
      (v0/*:: as any*/),
      {
        "alias": null,
        "args": null,
        "concreteType": "User",
        "kind": "LinkedField",
        "name": "user",
        "plural": false,
        "selections": [
          (v1/*:: as any*/)
        ],
        "storageKey": null
      }
    ],
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "HelpFormQuery",
    "selections": [
      (v0/*:: as any*/),
      {
        "alias": null,
        "args": null,
        "concreteType": "User",
        "kind": "LinkedField",
        "name": "user",
        "plural": false,
        "selections": [
          (v1/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "id",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ]
  },
  "params": {
    "cacheID": "341a1e6413cb88fd1d818049cd33b8cc",
    "id": null,
    "metadata": {},
    "name": "HelpFormQuery",
    "operationKind": "query",
    "text": "query HelpFormQuery {\n  feedbackAvailable\n  user {\n    email\n    id\n  }\n}\n"
  }
};
})();

(node as any).hash = "891106ed0447c55e1afb131d9671d1f6";

export default node;
