/**
 * @generated SignedSource<<3a17829ba97bb45dc00954e6f91c2826>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type SessionsListReactivateMutation$variables = {
  id: string;
};
export type SessionsListReactivateMutation$data = {
  readonly reactivateSession: {
    readonly id: string;
    readonly keyEnrolled: boolean;
    readonly keyExpiresAt: number | null | undefined;
    readonly lastContact: number | null | undefined;
    readonly reactivatable: boolean;
  };
};
export type SessionsListReactivateMutation = {
  response: SessionsListReactivateMutation$data;
  variables: SessionsListReactivateMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "defaultValue": null,
    "kind": "LocalArgument",
    "name": "id"
  }
],
v1 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      }
    ],
    "concreteType": "Session",
    "kind": "LinkedField",
    "name": "reactivateSession",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "keyEnrolled",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "keyExpiresAt",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "reactivatable",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "lastContact",
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
    "name": "SessionsListReactivateMutation",
    "selections": (v1/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "SessionsListReactivateMutation",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "41ff811ef4fa75f029ea3250c34f9d5b",
    "id": null,
    "metadata": {},
    "name": "SessionsListReactivateMutation",
    "operationKind": "mutation",
    "text": "mutation SessionsListReactivateMutation(\n  $id: ID!\n) {\n  reactivateSession(id: $id) {\n    id\n    keyEnrolled\n    keyExpiresAt\n    reactivatable\n    lastContact\n  }\n}\n"
  }
};
})();

(node as any).hash = "243dd7b0fd7d0e6d10f398f44ee2ef09";

export default node;
