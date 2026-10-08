/**
 * @generated SignedSource<<7b4a03906926b240b080726077b9a258>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
export type UserEditMutation$variables = {
  email: string;
  enabled: boolean;
  id: string;
  isDev: boolean;
  isSuper: boolean;
  locationGrants: ReadonlyArray<string>;
  readOnlyLocationGrants: ReadonlyArray<string>;
};
export type UserEditMutation$data = {
  readonly updateUser: {
    readonly email: string;
    readonly id: string;
    readonly isDev: boolean;
    readonly isSuper: boolean;
    readonly locationGrantIds: ReadonlyArray<string>;
    readonly readOnlyLocationGrantIds: ReadonlyArray<string>;
  };
};
export type UserEditMutation = {
  response: UserEditMutation$data;
  variables: UserEditMutation$variables;
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
v6 = {
  "defaultValue": null,
  "kind": "LocalArgument",
  "name": "readOnlyLocationGrants"
},
v7 = [
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
      },
      {
        "kind": "Variable",
        "name": "readOnlyLocationGrants",
        "variableName": "readOnlyLocationGrants"
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
        "name": "email",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "isSuper",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "isDev",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "locationGrantIds",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "readOnlyLocationGrantIds",
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [
      (v0/*:: as any*/),
      (v1/*:: as any*/),
      (v2/*:: as any*/),
      (v3/*:: as any*/),
      (v4/*:: as any*/),
      (v5/*:: as any*/),
      (v6/*:: as any*/)
    ],
    "kind": "Fragment",
    "metadata": null,
    "name": "UserEditMutation",
    "selections": (v7/*:: as any*/),
    "type": "MutationRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [
      (v2/*:: as any*/),
      (v0/*:: as any*/),
      (v4/*:: as any*/),
      (v3/*:: as any*/),
      (v5/*:: as any*/),
      (v6/*:: as any*/),
      (v1/*:: as any*/)
    ],
    "kind": "Operation",
    "name": "UserEditMutation",
    "selections": (v7/*:: as any*/)
  },
  "params": {
    "cacheID": "f86cbdc47f9321c833780e0c5893c921",
    "id": null,
    "metadata": {},
    "name": "UserEditMutation",
    "operationKind": "mutation",
    "text": "mutation UserEditMutation(\n  $id: ID!\n  $email: String!\n  $isSuper: Boolean!\n  $isDev: Boolean!\n  $locationGrants: [String!]!\n  $readOnlyLocationGrants: [String!]!\n  $enabled: Boolean!\n) {\n  updateUser(id: $id, email: $email, isSuper: $isSuper, isDev: $isDev, locationGrants: $locationGrants, readOnlyLocationGrants: $readOnlyLocationGrants, enabled: $enabled) {\n    id\n    email\n    isSuper\n    isDev\n    locationGrantIds\n    readOnlyLocationGrantIds\n  }\n}\n"
  }
};
})();

(node as any).hash = "139c15623d9297b893aa00ce8ef66735";

export default node;
