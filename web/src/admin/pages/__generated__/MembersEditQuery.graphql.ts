/**
 * @generated SignedSource<<b848e123ea2bdb2f44c817c47b775c89>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type MembersEditQuery$variables = {
  id: string;
};
export type MembersEditQuery$data = {
  readonly person: {
    readonly firstName: string;
    readonly id: string;
    readonly lastName: string;
    readonly memberNumber: string | null | undefined;
    readonly missingSince: number | null | undefined;
  };
};
export type MembersEditQuery = {
  response: MembersEditQuery$data;
  variables: MembersEditQuery$variables;
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
    "concreteType": "Person",
    "kind": "LinkedField",
    "name": "person",
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
        "name": "firstName",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "lastName",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "memberNumber",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "missingSince",
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
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "MembersEditQuery",
    "selections": (v1/*: any*/),
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*: any*/),
    "kind": "Operation",
    "name": "MembersEditQuery",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "3f7b9c5569adfd0cdd7e78cd958ee287",
    "id": null,
    "metadata": {},
    "name": "MembersEditQuery",
    "operationKind": "query",
    "text": "query MembersEditQuery(\n  $id: ID!\n) {\n  person(id: $id) {\n    id\n    firstName\n    lastName\n    memberNumber\n    missingSince\n  }\n}\n"
  }
};
})();

(node as any).hash = "4148416b28fd0e07fee5a045e0939949";

export default node;
