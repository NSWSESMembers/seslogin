/**
 * @generated SignedSource<<0d2ccf2494778e9b9bd1efff3c4fcc37>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type ApiTokenEditMutation$variables = {
  expiresAt?: number | null | undefined;
  id: string;
  locationGrants: ReadonlyArray<string>;
  name: string;
  readOnly: boolean;
};
export type ApiTokenEditMutation$data = {
  readonly updateApiToken: {
    readonly expiresAt: number | null | undefined;
    readonly id: string;
    readonly locationGrants: ReadonlyArray<string>;
    readonly name: string;
    readonly readOnly: boolean;
  };
};
export type ApiTokenEditMutation = {
  response: ApiTokenEditMutation$data;
  variables: ApiTokenEditMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "expiresAt"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "locationGrants"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "name"
},
v4 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "readOnly"
},
v5 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "expiresAt",
        "variableName": "expiresAt"
      },
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      },
      {
        "kind": "Variable",
        "name": "locationGrants",
        "variableName": "locationGrants"
      },
      {
        "kind": "Variable",
        "name": "name",
        "variableName": "name"
      },
      {
        "kind": "Variable",
        "name": "readOnly",
        "variableName": "readOnly"
      }
    ],
    "concreteType": "ApiToken",
    "kind": "LinkedField",
    "name": "updateApiToken",
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
        "name": "name",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "readOnly",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "locationGrants",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "expiresAt",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*: any*/),
      (v1/*: any*/),
      (v2/*: any*/),
      (v3/*: any*/),
      (v4/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "ApiTokenEditMutation",
    "selections": (v5/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v1/*: any*/),
      (v3/*: any*/),
      (v2/*: any*/),
      (v4/*: any*/),
      (v0/*: any*/)
    ],
    "kind": "Operation",
    "name": "ApiTokenEditMutation",
    "selections": (v5/*: any*/)
  },
  "params": {
    "cacheID": "ae63f9d2516121ccdfac3f72350815bb",
    "id": null,
    "metadata": {},
    "name": "ApiTokenEditMutation",
    "operationKind": "mutation",
    "text": "mutation ApiTokenEditMutation(\n  $id: ID!\n  $name: String!\n  $locationGrants: [String!]!\n  $readOnly: Boolean!\n  $expiresAt: Int\n) {\n  updateApiToken(id: $id, name: $name, locationGrants: $locationGrants, readOnly: $readOnly, expiresAt: $expiresAt) {\n    id\n    name\n    readOnly\n    locationGrants\n    expiresAt\n  }\n}\n"
  }
};
})();

(node as any).hash = "9e4976c3b115d043f001ea3f3cb00ab8";

export default node;
