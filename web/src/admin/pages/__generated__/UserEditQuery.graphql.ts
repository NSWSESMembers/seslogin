/**
 * @generated SignedSource<<5fcce7fb8cb4e023440e70d6324511a2>>
 * @lightSyntaxTransform
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest } from 'relay-runtime';
import { FragmentRefs } from "relay-runtime";
export type UserEditQuery$variables = {
  id: string;
};
export type UserEditQuery$data = {
  readonly locations: ReadonlyArray<{
    readonly id: string;
    readonly name: string;
  }>;
  readonly user: {
    readonly email: string;
    readonly enabled: boolean;
    readonly id: string;
    readonly isDev: boolean;
    readonly isSuper: boolean;
    readonly locationGrantIds: ReadonlyArray<string>;
    readonly oauthGrants: ReadonlyArray<{
      readonly clientName: string;
      readonly createdAt: number;
      readonly id: string;
      readonly isFirstParty: boolean;
      readonly lastUsedAt: number | null | undefined;
      readonly redirectHost: string;
    }>;
    readonly readOnlyLocationGrantIds: ReadonlyArray<string>;
    readonly " $fragmentSpreads": FragmentRefs<"UserMemberField_user">;
  };
};
export type UserEditQuery = {
  response: UserEditQuery$data;
  variables: UserEditQuery$variables;
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
    "kind": "Variable",
    "name": "id",
    "variableName": "id"
  }
],
v2 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v3 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "email",
  "storageKey": null
},
v4 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "isSuper",
  "storageKey": null
},
v5 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "isDev",
  "storageKey": null
},
v6 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "enabled",
  "storageKey": null
},
v7 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "locationGrantIds",
  "storageKey": null
},
v8 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "readOnlyLocationGrantIds",
  "storageKey": null
},
v9 = {
  "alias": null,
  "args": null,
  "concreteType": "OauthGrant",
  "kind": "LinkedField",
  "name": "oauthGrants",
  "plural": true,
  "selections": [
    (v2/*:: as any*/),
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "clientName",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "redirectHost",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "isFirstParty",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "createdAt",
      "storageKey": null
    },
    {
      "alias": null,
      "args": null,
      "kind": "ScalarField",
      "name": "lastUsedAt",
      "storageKey": null
    }
  ],
  "storageKey": null
},
v10 = [
  (v2/*:: as any*/),
  {
    "alias": null,
    "args": null,
    "kind": "ScalarField",
    "name": "name",
    "storageKey": null
  }
],
v11 = {
  "alias": null,
  "args": null,
  "concreteType": "Location",
  "kind": "LinkedField",
  "name": "locations",
  "plural": true,
  "selections": (v10/*:: as any*/),
  "storageKey": null
};
return {
  "fragment": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Fragment",
    "metadata": {
      "throwOnFieldError": true
    },
    "name": "UserEditQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*:: as any*/),
        "concreteType": "User",
        "kind": "LinkedField",
        "name": "user",
        "plural": false,
        "selections": [
          (v2/*:: as any*/),
          (v3/*:: as any*/),
          (v4/*:: as any*/),
          (v5/*:: as any*/),
          (v6/*:: as any*/),
          (v7/*:: as any*/),
          (v8/*:: as any*/),
          {
            "args": null,
            "kind": "FragmentSpread",
            "name": "UserMemberField_user"
          },
          (v9/*:: as any*/)
        ],
        "storageKey": null
      },
      (v11/*:: as any*/)
    ],
    "type": "QueryRoot",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": (v0/*:: as any*/),
    "kind": "Operation",
    "name": "UserEditQuery",
    "selections": [
      {
        "alias": null,
        "args": (v1/*:: as any*/),
        "concreteType": "User",
        "kind": "LinkedField",
        "name": "user",
        "plural": false,
        "selections": [
          (v2/*:: as any*/),
          (v3/*:: as any*/),
          (v4/*:: as any*/),
          (v5/*:: as any*/),
          (v6/*:: as any*/),
          (v7/*:: as any*/),
          (v8/*:: as any*/),
          {
            "alias": null,
            "args": null,
            "concreteType": "Person",
            "kind": "LinkedField",
            "name": "person",
            "plural": false,
            "selections": [
              (v2/*:: as any*/),
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
                "concreteType": "Location",
                "kind": "LinkedField",
                "name": "location",
                "plural": false,
                "selections": (v10/*:: as any*/),
                "storageKey": null
              }
            ],
            "storageKey": null
          },
          (v9/*:: as any*/)
        ],
        "storageKey": null
      },
      (v11/*:: as any*/)
    ]
  },
  "params": {
    "cacheID": "526e9352357a2d8ab08cc036efea55ca",
    "id": null,
    "metadata": {},
    "name": "UserEditQuery",
    "operationKind": "query",
    "text": "query UserEditQuery(\n  $id: ID!\n) {\n  user(id: $id) {\n    id\n    email\n    isSuper\n    isDev\n    enabled\n    locationGrantIds\n    readOnlyLocationGrantIds\n    ...UserMemberField_user\n    oauthGrants {\n      id\n      clientName\n      redirectHost\n      isFirstParty\n      createdAt\n      lastUsedAt\n    }\n  }\n  locations {\n    id\n    name\n  }\n}\n\nfragment UserMemberField_user on User {\n  id\n  person {\n    id\n    firstName\n    lastName\n    memberNumber\n    location {\n      id\n      name\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "bb8febc2230423f0518eddcb03b0440d";

export default node;
