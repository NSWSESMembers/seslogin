/**
 * @generated SignedSource<<be0f7fa415e8922c9e5390a5e7041bb3>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type UserListToggleMutation$variables = {
  email: string;
  enabled: boolean;
  id: string;
  isDev: boolean;
  isSuper: boolean;
  locationGrants: ReadonlyArray<string>;
};
export type UserListToggleMutation$data = {
  readonly updateUser: {
    readonly enabled: boolean;
    readonly id: string;
  };
};
export type UserListToggleMutation = {
  response: UserListToggleMutation$data;
  variables: UserListToggleMutation$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "email"
},
v1 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "enabled"
},
v2 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "id"
},
v3 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "isDev"
},
v4 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "isSuper"
},
v5 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "locationGrants"
},
v6 = [
  {
    "alias": null,
    "args": [
      {
        "kind": "Variable",
        "name": "email",
        "variableName": "email"
      },
      {
        "kind": "Variable",
        "name": "enabled",
        "variableName": "enabled"
      },
      {
        "kind": "Variable",
        "name": "id",
        "variableName": "id"
      },
      {
        "kind": "Variable",
        "name": "isDev",
        "variableName": "isDev"
      },
      {
        "kind": "Variable",
        "name": "isSuper",
        "variableName": "isSuper"
      },
      {
        "kind": "Variable",
        "name": "locationGrants",
        "variableName": "locationGrants"
      }
    ],
    "concreteType": "User",
    "kind": "LinkedField",
    "name": "updateUser",
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
        "name": "enabled",
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
      (v4/*: any*/),
      (v5/*: any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "UserListToggleMutation",
    "selections": (v6/*: any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v2/*: any*/),
      (v0/*: any*/),
      (v4/*: any*/),
      (v3/*: any*/),
      (v5/*: any*/),
      (v1/*: any*/)
    ],
    "kind": "Operation",
    "name": "UserListToggleMutation",
    "selections": (v6/*: any*/)
  },
  "params": {
    "cacheID": "f189cb4338d44fea75a6e19e0a74f1b0",
    "id": null,
    "metadata": {},
    "name": "UserListToggleMutation",
    "operationKind": "mutation",
    "text": "mutation UserListToggleMutation(\n  $id: ID!\n  $email: String!\n  $isSuper: Boolean!\n  $isDev: Boolean!\n  $locationGrants: [String!]!\n  $enabled: Boolean!\n) {\n  updateUser(id: $id, email: $email, isSuper: $isSuper, isDev: $isDev, locationGrants: $locationGrants, enabled: $enabled) {\n    id\n    enabled\n  }\n}\n"
  }
};
})();

(node as any).hash = "e559ab0561296c94690c6d667be27903";

export default node;
