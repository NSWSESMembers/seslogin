/**
 * @generated SignedSource<<9d2058db1de54c8574f4ca154dac578e>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SessionEnrollQuery$variables = {
  fingerprint: string;
};
export type SessionEnrollQuery$data = {
  readonly pendingEnrollmentKey: {
    readonly __typename: "PendingEnrollmentKey";
  } | null | undefined;
};
export type SessionEnrollQuery = {
  response: SessionEnrollQuery$data;
  variables: SessionEnrollQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "fingerprint"
  }
],
v1 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "fingerprint",
        "variableName": "fingerprint"
      }
    ],
    "concreteType": "PendingEnrollmentKey",
    "kind": "LinkedField",
    "name": "pendingEnrollmentKey",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "__typename",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Fragment",
    "metadata": null,
    "name": "SessionEnrollQuery",
    "selections": (v1/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "SessionEnrollQuery",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "267b465c0fde66fe5d1839547da7997b",
    "id": null,
    "metadata": {},
    "name": "SessionEnrollQuery",
    "operationKind": "query",
    "text": "query SessionEnrollQuery(\n  $fingerprint: String!\n) {\n  pendingEnrollmentKey(fingerprint: $fingerprint) {\n    __typename\n  }\n}\n"
  }
};
})();

(node as any).hash = "11667855956eeeae637e9899bb3664d5";

export default node;
